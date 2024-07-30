use std::{future::Future, hint::spin_loop, ptr::{addr_of_mut, null_mut, NonNull}, sync::{atomic::{AtomicPtr, AtomicU8, AtomicUsize, Ordering}, Arc, Mutex}, task::{Poll, Waker}};

/// A state in which the token has not yet been awoken. In this state, wakers
/// can be added to the `waker` map and they will be awoken if the state
/// changes to `STATE_AWAKE`.
const STATE_WAIT: u8 = 0;
/// A state in which the token has been awoken. No additional wakers should be
/// added to `wakers`.
const STATE_AWAKE: u8 = 1;

#[repr(u8)]
enum State {
    /// Equivalent to `STATE_WAIT`
    Wait = STATE_WAIT,
    /// Equivalent to `STATE_AWAKE`
    Awake = STATE_AWAKE,
}

impl From<u8> for State {
    #[inline]
    fn from(value: u8) -> Self {
        match value {
            STATE_WAIT => State::Wait,
            STATE_AWAKE => State::Awake,
            err_state => panic!("The awake token was in a state of neither being WAIT ({STATE_WAIT}) nor AWAKE ({STATE_AWAKE}). State was {err_state}"),
        }
    }
}

#[derive(Debug)]
struct ALinkedList {
    head: NeighborPtr,
    tail: AtomicPtr<AwokenState>,
}

#[derive(Debug)]
pub struct AwakeToken {
    state: AtomicU8,
    wakers: ALinkedList,
}

impl AwakeToken {
    #[inline]
    pub fn new() -> Self {
        Self {
            state: AtomicU8::new(State::Wait as u8),
            wakers: ALinkedList {
                head: NeighborPtr::new(),
                tail: AtomicPtr::new(null_mut()),
            },
        }
    }

    #[inline]
    pub fn awake(&self) {
        match State::from(self.state.swap(State::Awake as u8, Ordering::AcqRel)) {
            State::Wait => {
                let mut next_current_state = self.wakers.head.follow();
                while let Some(current_state) = next_current_state {
                    let l_waker = current_state.waker().lock().unwrap();
                    l_waker.wake_by_ref();
                    drop(l_waker);
                    next_current_state = current_state.right().follow();
                }
            },
            State::Awake => (), //< Already awake, should not be awoken twice.
        };
    }

    #[inline]
    pub fn awoken(self: Arc<Self>) -> AwokenToken {
        match State::from(self.state.load(Ordering::Acquire)) {
            State::Wait => AwokenToken { state: AwokenState::Fresh { awake_token: self } },
            State::Awake => AwokenToken { state: AwokenState::Awoken },
        }
    }

    #[inline]
    pub fn try_awoken(&self) -> bool {
        match State::from(self.state.load(Ordering::Acquire)) {
            State::Wait => false,
            State::Awake => true,
        }
    }
}

#[derive(Debug)]
struct NeighborPtr {
    /// The number of nodes actively following this pointer who have not yet updated the reference
    /// count at the node it points to. This will prevent this pointer from being dropped before the
    /// neighbor's reference count can be incremented.
    in_use: AtomicUsize,
    /// A pointer to a neighboring node. Can be null.
    neighbor: AtomicPtr<AwokenState>,
}

impl NeighborPtr {
    /// Creates a struct representing a pointer to a neighbor. The `neighbor` is null and `in_use`
    /// is set to `0`.
    fn new() -> Self {
        Self {
            in_use: AtomicUsize::new(0),
            neighbor: AtomicPtr::new(null_mut()),
        }
    }

    /// Follows the neighbor pointer safely, incrementing and decrementing reference counts as
    /// needed.
    fn follow(&self) -> Option<ArcAwokenState>{
        self.in_use.fetch_add(1, Ordering::Release);
        let arc_ptr = match NonNull::new(self.neighbor.load(Ordering::Acquire)) {
            Some(neighbor_ptr) => {
                match unsafe { neighbor_ptr.as_ref() } {
                    AwokenState::Registered { awake_token: _, waker: _, left: _, ref_count: neighbor_ref_count, right: _ } => {
                        neighbor_ref_count.fetch_add(1, Ordering::Release);
                    },
                    AwokenState::Fresh { awake_token: _ } => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Fresh"),
                    AwokenState::Awoken => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Awoken"),
                }
                Some(ArcAwokenState { ptr: neighbor_ptr })
            },
            None => None,
        };
        self.in_use.fetch_sub(1, Ordering::Release);
        arc_ptr
    }

    /// Wait for this pointer to not be in use. This is used to prevent anyone from getting stuck in
    /// a state where they have loaded the pointer but then the thing it pointed to got deallocated.
    /// After changing the the underlying pointer, this should be called to make sure everyone has
    /// acquired their proper reference counts.
    /// It's worth noting that this may result in you waiting on people referencing whatever you
    /// swapped this neighbor ptr with. But this is safer than the alternative where you deallocate
    /// memory and then they try to reference it.
    fn wait(&self) {
        while self.in_use.load(Ordering::Acquire) > 0 {
            spin_loop();
        }
    }
}

fn state_waker(state: &NonNull<AwokenState>) -> &Mutex<Waker> {
    match unsafe { state.as_ref() } {
        AwokenState::Fresh { awake_token: _ } => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Fresh"),
        AwokenState::Registered { awake_token: _, waker, left: _, ref_count: _, right: _ } => waker,
        AwokenState::Awoken => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Awoken"),
    }
}

fn state_ref_count(state: &NonNull<AwokenState>) -> &AtomicUsize {
    match unsafe { state.as_ref() } {
        AwokenState::Fresh { awake_token: _ } => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Fresh"),
        AwokenState::Registered { awake_token: _, waker: _, left: _, ref_count, right: _ } => ref_count,
        AwokenState::Awoken => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Awoken"),
    }
}

fn state_left(state: &NonNull<AwokenState>) -> &AtomicPtr<AwokenState> {
    match unsafe { state.as_ref() } {
        AwokenState::Fresh { awake_token: _ } => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Fresh"),
        AwokenState::Registered { awake_token: _, waker: _, left, ref_count: _, right: _ } => left,
        AwokenState::Awoken => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Awoken"),
    }
}

fn state_right(state: &NonNull<AwokenState>) -> &NeighborPtr {
    match unsafe { state.as_ref() } {
        AwokenState::Fresh { awake_token: _ } => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Fresh"),
        AwokenState::Registered { awake_token: _, waker: _, left: _, ref_count: _, right } => right,
        AwokenState::Awoken => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Awoken"),
    }
}

/// A reference counted pointer to a node in the ALinkedList. Where possible, this is used to ensure
/// that reference counts are always decremented when a reference counted pointer is dropped.
struct ArcAwokenState {
    ptr: NonNull<AwokenState>
}

impl ArcAwokenState {
    fn waker(&self) -> &Mutex<Waker> {
        state_waker(&self.ptr)
    }

    fn ref_count(&self) -> &AtomicUsize {
        state_ref_count(&self.ptr)
    }

    fn left(&self) -> &AtomicPtr<AwokenState> {
        state_left(&self.ptr)
    }

    fn right(&self) -> &NeighborPtr {
        state_right(&self.ptr)
    }
}

impl Drop for ArcAwokenState {
    fn drop(&mut self) {
        match unsafe { self.ptr.as_ref() } {
            AwokenState::Registered { awake_token: _, waker: _, left: _, ref_count: self_ref_count, right: _ } => {
                self_ref_count.fetch_sub(1, Ordering::Release);
            },
            AwokenState::Fresh { awake_token: _ } => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Fresh"),
            AwokenState::Awoken => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Awoken"),
        }
    }
}

#[derive(Debug)]
enum AwokenState {
    Fresh { awake_token: Arc<AwakeToken> },
    Registered {
        /// Points back to the AwakeToken that this token is registered with. Storing an Arc of the
        /// AwakeToken with the state also ensures that the linked list cannot be dropped before all
        /// the registered nodes that make up the linked list.
        awake_token: Arc<AwakeToken>,
        /// The waker used to wake up this token. A mutex is needed to ensure that the waker cannot
        /// be called at the same time that a new waker is being assigned.
        waker: Mutex<Waker>,

        /// Points to the neighbor to the left of this node (towards the head).
        /// If this node is fully initialized and the pointer is null, then this node is the head.
        /// The linked list's head pointer might be null (not point at this node) during some
        /// transition times so this is the only reliable way to know if this node is the head.
        /// This pointer can only be used by the node that owns it.
        left: AtomicPtr<AwokenState>,
        /// Counts the number of other pointers to this node. When all paths into this node have
        /// been removed and this counter has reached zero, it is safe to drop or replace this
        /// memory.
        ref_count: AtomicUsize,
        /// Points to the neighbor to the right of this node (towards the tail).
        /// If this node is fully initialized and this pointer is null, then this node is a tail.
        /// There may be nodes after this one which are still initializing. The Linked List's tail
        /// pointer might point at the last of those nodes if they exist.
        /// Even if the LinkedList's tail pointer points to this node, it may not be reachable using
        /// `for_each()` if a node somewhere to the left of this one is still initializing.
        right: NeighborPtr
    },
    Awoken,
}

impl AwokenState {
    fn waker(&self) -> &Mutex<Waker> {
        match &self {
            AwokenState::Fresh { awake_token: _ } => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Fresh"),
            AwokenState::Registered { awake_token: _, waker, left: _, ref_count: _, right: _ } => waker,
            AwokenState::Awoken => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Awoken"),
        }
    }
    
    fn ref_count(&self) -> &AtomicUsize {
        match &self {
            AwokenState::Fresh { awake_token: _ } => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Fresh"),
            AwokenState::Registered { awake_token: _, waker: _, left: _, ref_count, right: _ } => ref_count,
            AwokenState::Awoken => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Awoken"),
        }
    }
    
    fn awake_token(&self) -> &AwakeToken {
        match &self {
            AwokenState::Fresh { awake_token: _ } => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Fresh"),
            AwokenState::Registered { awake_token, waker: _, left: _, ref_count: _, right: _ } => awake_token,
            AwokenState::Awoken => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Awoken"),
        }
    }
    
    fn left(&self) -> &AtomicPtr<AwokenState> {
        match &self {
            AwokenState::Fresh { awake_token: _ } => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Fresh"),
            AwokenState::Registered { awake_token: _, waker: _, left, ref_count: _, right: _ } => left,
            AwokenState::Awoken => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Awoken"),
        }
    }
    
    fn right(&self) -> &NeighborPtr {
        match &self {
            AwokenState::Fresh { awake_token: _ } => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Fresh"),
            AwokenState::Registered { awake_token: _, waker: _, left: _, ref_count: _, right } => right,
            AwokenState::Awoken => panic!("The AwokenToken's state must be Registered to be a part of the linked list but it was Awoken"),
        }
    }
}

#[derive(Debug)]
pub struct AwokenToken {
    state: AwokenState
}

impl AwokenToken {
    /// Removes self from the AwakeToken's linked list of registered tokens.
    fn remove(&mut self) {
        let self_addr = addr_of_mut!(self.state);
        let awake_token = self.state.awake_token();

        // If left is Some, then dropping it requires decrementing the reference count at the node
        // it points to.
        match NonNull::new(self.state.left().swap(null_mut(), Ordering::AcqRel)) {
            Some(self_left) => {
                let mut removed_refs = 0;

                // If the node to our right is also doing a `remove()` operation, we should wait for
                // them to update our right pointer.
                let self_right;
                loop {
                    if let Some(self_right_current) = self.state.right().follow() {
                        if let Ok(_) = self_right_current.left().compare_exchange(self_addr, self_left.as_ptr(), Ordering::AcqRel, Ordering::Relaxed) {
                            // `next` now points to `left` instead of us.
                            // It's ok to post-increment the ref count because we our holding a
                            // reference to `left` that is reference counted so it cannot be
                            // deallocated before we drop that reference.
                            state_ref_count(&self_left).fetch_add(1, Ordering::AcqRel);
                            removed_refs += 1;
                            self_right = Some(self_right_current);
                            break;
                        }
                        drop(self_right_current);
                    // If the node to our right was the tail, we may become the new tail.
                    // We should try to make node before us the tail instead if that occurs.
                    } else if let Ok(_) = awake_token.wakers.tail.compare_exchange(self_addr, self_left.as_ptr(), Ordering::AcqRel, Ordering::Relaxed) {
                        // `tail` now has pointer to `left` instead of us.
                        // It's ok to post-increment the ref count because we our holding a
                        // reference to `left` that is reference counted so it cannot be deallocated
                        // before we drop that reference.
                        state_ref_count(&self_left).fetch_add(1, Ordering::AcqRel);
                        removed_refs += 1;
                        self_right = None;
                        break;
                    }
                    spin_loop();
                }

                match self_right {
                    Some(self_right) => {
                        // Because you can acquire self_left while the node directly to your left is
                        // still being removed, your `self_left` variable may point to a node that
                        // actually several nodes before you still.
                        // Need to wait until it points to yourself before you can move on.
                        self_right.ref_count().fetch_add(1, Ordering::AcqRel);
                        while let Err(_) = state_right(&self_left).neighbor.compare_exchange(self_addr, self_right.ptr.as_ptr(), Ordering::AcqRel, Ordering::Relaxed) {
                            spin_loop();
                        }
                        removed_refs += 1;

                        state_ref_count(&self_left).fetch_sub(1, Ordering::AcqRel);
                        let _ = self_left;  //< We decremented ref count. No longer safe to us.

                        // At this point, there is no way to reach us from our neighbors.
                        // `head` and `tail` cannot point to us because we were an interior node and
                        // we modified references to our left and right to skip us.

                        // Wait for anybody actively iterating through us to leave.
                        while self.state.ref_count().load(Ordering::Acquire) > removed_refs {
                            spin_loop();
                        }
                        // Wait for anybody actively iterating through us to have made it to the
                        // next node.
                        self.state.right().wait();

                        // Clear paths to reach our neighbors.
                        self.state.right().neighbor.store(null_mut(), Ordering::Release);
                        // Clear reference count from our `right` value.
                        self_right.ref_count().fetch_sub(1, Ordering::AcqRel);
                        // Reference count from `self_right` is automatically dealt with.
                        drop(self_right);
                    },
                    None => {
                        // Because you can acquire self_left while the node directly to your left is
                        // still being removed, your `self_left` variable may point to a node that
                        // actually several nodes before you still.
                        // Need to wait until it points to yourself before you can move on.
                        while let Err(_) = state_right(&self_left).neighbor.compare_exchange(self_addr, null_mut(), Ordering::AcqRel, Ordering::Relaxed) {
                            spin_loop();
                        }
                        removed_refs += 1;

                        state_ref_count(&self_left).fetch_sub(1, Ordering::AcqRel);
                        let _ = self_left;  //< We decremented ref count. No longer safe to us.

                        // At this point, there is no way to reach us from our neighbors.
                        // `head` and `tail` cannot point to us because we were not the leftmost
                        // node and if `tail` did point to us, that was swapped during the first
                        // spin lock.

                        // Wait for anybody actively iterating through us to leave.
                        while self.state.ref_count().load(Ordering::Acquire) > removed_refs {
                            spin_loop();
                        }
                        // Wait for anybody actively iterating through us to have made it to the
                        // next node.
                        self.state.right().wait();

                        // No neighbors to our right so we are all done.
                    },
                }
            },
            // Nobody to the left = we are head of list. Special case.
            None => {
                let mut removed_refs = 0;

                // If the node to our right is also doing a `remove()` operation, we should
                // wait for them to update our right pointer.
                let self_right;
                loop {
                    if let Some(self_right_current) = self.state.right().follow() {
                        if let Ok(_) = self_right_current.left().compare_exchange(self_addr, null_mut(), Ordering::AcqRel, Ordering::Relaxed) {
                            // `next` now points to null instead of us. This makes them the
                            // new head, although we have not yet updated the `head` to
                            // reflect this change.
                            removed_refs += 1;
                            self_right = Some(self_right_current);
                            break;
                        }
                        drop(self_right_current);
                    // If the node to our right was the tail, we may become the new tail.
                    // We should try to make node before us the tail instead if that occurs.
                    } else if let Ok(_) = awake_token.wakers.tail.compare_exchange(self_addr, null_mut(), Ordering::AcqRel, Ordering::Relaxed) {
                        // `tail` now has pointer to null instead of us.
                        removed_refs += 1;
                        self_right = None;
                        break;
                    }
                    spin_loop();
                }

                match self_right {
                    Some(self_right) => {
                        while let Err(_) = awake_token.wakers.head.neighbor.compare_exchange(self_addr, self_right.ptr.as_ptr(), Ordering::AcqRel, Ordering::Relaxed) {
                            spin_loop();
                        }
                        removed_refs += 1;

                        // At this point, there is no way to reach us from our neighbors.

                        // Wait for anybody actively acquiring a pointer to us to have
                        // acquired a strong reference count.
                        awake_token.wakers.head.wait();
                        // Wait for anybody actively iterating through us to leave.
                        while self.state.ref_count().load(Ordering::Acquire) > removed_refs {
                            spin_loop();
                        }
                        // Wait for anybody actively iterating through us to have made it to
                        // the next node.
                        self.state.right().wait();

                        // Clear paths to reach our neighbors.
                        self.state.right().neighbor.store(null_mut(), Ordering::Release);
                        // Clear reference count from our `right` value.
                        self_right.ref_count().fetch_sub(1, Ordering::AcqRel);
                        // Reference count from `self_right` is automatically dealt with.
                        drop(self_right);
                    },
                    None => {
                        while let Err(_) = awake_token.wakers.head.neighbor.compare_exchange(self_addr, null_mut(), Ordering::AcqRel, Ordering::Relaxed) {
                            spin_loop();
                        }
                        removed_refs += 1;

                        // At this point, there is no way to reach us from our neighbors.

                        // Wait for anybody actively acquiring a pointer to us to have
                        // acquired a strong reference count.
                        awake_token.wakers.head.wait();
                        // Wait for anybody actively iterating through us to leave.
                        while self.state.ref_count().load(Ordering::Acquire) > removed_refs {
                            spin_loop();
                        }
                        // Wait for anybody actively iterating through us to have made it to
                        // the next node (in this case, null).
                        self.state.right().wait();

                        // No neighbors to our right so we are all done.
                    },
                }
            }
        }
    }

    /// Awakes any tokens to the right of this token. This is used to ensure that all tokens to the
    /// right are awoken if the list is not in a fully consistent state.
    /// Assumes that you have already awoken yourself.
    fn awake_right(&self) {
        if let Some(mut self_right_state) = self.state.right().follow() {
            while let Some(rights_right_state) = self_right_state.right().follow() {
                let l_rights_waker = rights_right_state.waker().lock().unwrap();
                l_rights_waker.wake_by_ref();
                drop(l_rights_waker);
                self_right_state = rights_right_state;
            }
        }
    }
}

impl<'a> Future for AwokenToken {
    type Output = ();

    #[inline]
    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match &self.state {
            AwokenState::Fresh { awake_token } => match State::from(awake_token.state.load(Ordering::Acquire)) {
                State::Wait => {
                    let awake_token = awake_token.clone();
                    self.state = AwokenState::Registered {
                        awake_token: awake_token.clone(),
                        waker: Mutex::new(cx.waker().clone()),
                        left: AtomicPtr::new(null_mut()),
                        // Although we have not yet added the pointers that these reference
                        // counts are accounting for, we will shortly. This makes it so we don't
                        // need to add them later.
                        // If we are the head, the pointers will be:
                        //  tail->self
                        //  head->self
                        // If we are a tail, the pointer will be:
                        //  tail->self
                        //  left_neighbor->self
                        ref_count: AtomicUsize::new(2),
                        right: NeighborPtr::new(),
                    };
                    let self_addr = addr_of_mut!(self.state);

                    // We own this pointer to our left neighbor even though it has not yet been
                    // added to our left pointer. Our neighbor's reference count already accounts
                    // for this reference since we took it directly from `tail`.
                    let left_neighbor_ptr = NonNull::new(awake_token.wakers.tail.swap(self_addr, Ordering::AcqRel));
                    match left_neighbor_ptr {
                        // Special case: If the tail was null, that means we are the first node
                        // in the list. Aka. we can also store our address for head. This brings
                        // us to a consistent state early.
                        None => {
                            // Note: Reference count accounted for at start.
                            while let Err(_) = awake_token.wakers.head.neighbor.compare_exchange(null_mut(), self_addr, Ordering::AcqRel, Ordering::Relaxed) {
                                spin_loop();
                            }
                        },
                        // Normal case: There is a neighbor to our left that we need to finish
                        // connecting  ourself to.
                        Some(left_neighbor_ptr) => {
                            // Note: Reference counts accounted for at start.
                            self.state.left().store(left_neighbor_ptr.as_ptr(), Ordering::Release);
                            // Now need to add ourself as their neighbor.
                            while let Err(_) = state_right(&left_neighbor_ptr).neighbor.compare_exchange(null_mut(), self_addr, Ordering::AcqRel, Ordering::Relaxed) {
                                spin_loop();
                            }
                        },
                    }

                    // Consistent State Reached.

                    // Need to verify that the awake token wasn't awoken. Otherwise, we need to wake
                    // ourselves and any neighbors to our right (in case the neighbors to our left
                    // don't reach the true head)
                    match State::from(awake_token.state.load(Ordering::Acquire)) {
                        // State Change: Fresh --> Registered
                        State::Wait => Poll::Pending,
                        // State Change: Fresh --> Registered --> Awoken
                        State::Awake => {
                            self.awake_right();
                            self.remove();
                            // Once the token is fully isolated, it is safe to modify the state.
                            self.state = AwokenState::Awoken;
                            Poll::Ready(())

                        },
                    }
                },
                State::Awake => {
                    // State Change: Fresh --> Awoken
                    self.state = AwokenState::Awoken;
                    Poll::Ready(())
                },
            },
            AwokenState::Registered { awake_token, waker, left: _, ref_count: _, right: _ } => match State::from(awake_token.state.load(Ordering::Acquire)) {
                // There is a case where the future gets re-polled, but is still alive.
                State::Wait => {
                    let mut l_waker = waker.lock().unwrap();
                    l_waker.clone_from(cx.waker());
                    drop(l_waker);

                    // Need to verify that the awake token wasn't awoken. Otherwise, we need to wake
                    // ourselves.
                    // Don't need to awake nodes to the right because we are starting from a stable
                    // state (aka. There are nodes to our left who will awake to the right or the
                    // head will awake right).
                    match State::from(awake_token.state.load(Ordering::Acquire)) {
                        // State Change: Registered --> Registered
                        State::Wait => Poll::Pending,
                        // State Change: Registered --> Registered --> Awoken
                        State::Awake => {
                            self.remove();
                            // Once the token is fully isolated, it is safe to modify the state.
                            self.state = AwokenState::Awoken;
                            Poll::Ready(())

                        },
                    }
                },
                State::Awake => {
                    // Don't need to awake nodes to the right because we are starting from a stable
                    // state (aka. There are nodes to our left who will awake to the right or the
                    // head will awake right).
                    self.remove();
                    // State Change: Registered --> Awoken
                    self.state = AwokenState::Awoken;
                    Poll::Ready(())
                },
            },
            AwokenState::Awoken => Poll::Ready(()),
        }
    }
}

impl Drop for AwokenToken {
    #[inline]
    fn drop(&mut self) {
        match &self.state {
            AwokenState::Fresh { awake_token: _ } => (),
            AwokenState::Registered { awake_token: _, waker: _, left: _, ref_count: _, right: _ } => {
                self.remove();
            },
            AwokenState::Awoken => (),
        };
    }
}
