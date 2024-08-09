use std::{future::Future, hint::spin_loop, mem, ptr::{addr_of_mut, null_mut, NonNull}, sync::{atomic::{AtomicPtr, AtomicU8, AtomicUsize, Ordering}, Arc, Mutex}, task::{Poll, Waker}};

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
    tail: AtomicArcAwokenState,
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
                tail: AtomicArcAwokenState::new_null(),
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
        match State::from(self.state.load(Ordering::Relaxed)) {
            State::Wait => AwokenToken { state: AwokenState::Fresh { awake_token: self } },
            State::Awake => AwokenToken { state: AwokenState::Awoken },
        }
    }

    #[inline]
    pub fn try_awoken(&self) -> bool {
        match State::from(self.state.load(Ordering::Relaxed)) {
            State::Wait => false,
            State::Awake => true,
        }
    }
}

trait RegisteredState {
    fn waker(&self) -> &Mutex<Waker>;
    fn ref_count(&self) -> &AtomicUsize;
    fn awake_token(&self) -> &AwakeToken;
    fn left(&self) -> &AtomicArcAwokenState;
    fn right(&self) -> &NeighborPtr;
}

trait UnsafeRegisteredState {
    unsafe fn waker(&self) -> &Mutex<Waker>;
    unsafe fn ref_count(&self) -> &AtomicUsize;
    unsafe fn awake_token(&self) -> &AwakeToken;
    unsafe fn left(&self) -> &AtomicArcAwokenState;
    unsafe fn right(&self) -> &NeighborPtr;
}

impl UnsafeRegisteredState for NonNull<AwokenState> {
    unsafe fn waker(&self) -> &Mutex<Waker> {
        unsafe { self.as_ref() }.waker()
    }

    unsafe fn ref_count(&self) -> &AtomicUsize {
        unsafe { self.as_ref() }.ref_count()
    }

    unsafe fn awake_token(&self) -> &AwakeToken {
        unsafe { self.as_ref() }.awake_token()
    }

    unsafe fn left(&self) -> &AtomicArcAwokenState {
        unsafe { self.as_ref() }.left()
    }

    unsafe fn right(&self) -> &NeighborPtr {
        unsafe { self.as_ref() }.right()
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
        left: AtomicArcAwokenState,
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

impl RegisteredState for AwokenState {
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

    fn left(&self) -> &AtomicArcAwokenState {
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

/// A reference counted pointer to a node in the ALinkedList. Where possible, this is used to ensure
/// that reference counts are always decremented when a reference counted pointer is dropped.
#[derive(Debug)]
struct ArcAwokenState {
    ptr: NonNull<AwokenState>
}

impl ArcAwokenState {
    /// The pointer must be associated with a reference count. Creating the
    /// ArcAwokenState from a pointer passes the responsibility of decrementing
    /// the reference count to the ArcAwokenState.
    unsafe fn from_ptr(ptr: NonNull<AwokenState>) -> Self { Self { ptr } }
    fn as_ptr(&self) -> *mut AwokenState { self.ptr.as_ptr() }
    fn as_non_null(&self) -> NonNull<AwokenState> { self.ptr }
}

impl RegisteredState for ArcAwokenState {
    fn waker(&self) -> &Mutex<Waker> {
        // The pointer is reference counted. As long as we hold this reference,
        // the value is guaranteed to exist.
        unsafe { self.ptr.waker() }
    }

    fn ref_count(&self) -> &AtomicUsize {
        // The pointer is reference counted. As long as we hold this reference,
        // the value is guaranteed to exist.
        unsafe { self.ptr.ref_count() }
    }

    fn awake_token(&self) -> &AwakeToken {
        // The pointer is reference counted. As long as we hold this reference,
        // the value is guaranteed to exist.
        unsafe { self.ptr.awake_token() }
    }

    fn left(&self) -> &AtomicArcAwokenState {
        // The pointer is reference counted. As long as we hold this reference,
        // the value is guaranteed to exist.
        unsafe { self.ptr.left() }
    }

    fn right(&self) -> &NeighborPtr {
        // The pointer is reference counted. As long as we hold this reference,
        // the value is guaranteed to exist.
        unsafe { self.ptr.right() }
    }
}

impl Clone for ArcAwokenState {
    fn clone(&self) -> Self {
        self.ref_count().fetch_add(1, Ordering::AcqRel);
        unsafe { Self::from_ptr(self.ptr) }
    }
}

impl Drop for ArcAwokenState {
    fn drop(&mut self) {
        self.ref_count().fetch_sub(1, Ordering::Release);
    }
}

/// A reference counted pointer to a node in the ALinkedList. Where possible, this is used to ensure
/// that reference counts are always decremented when a reference counted pointer is dropped.
/// Unlike ArcAwokenState, this is intended for cases where the underlying pointer can be changed
/// at any time. Fewer operations are provided because operations like `load` are inherently unsafe.
#[derive(Debug)]
struct AtomicArcAwokenState {
    ptr: AtomicPtr<AwokenState>
}

impl AtomicArcAwokenState {
    fn new_null() -> Self { Self { ptr: AtomicPtr::new(null_mut()) } }

    /// The pointer must be associated with a reference count if it is
    /// non-null. Creating themArcAwokenState from a pointer passes the
    /// responsibility of decrementing the reference count to the
    /// ArcAwokenState.
    unsafe fn from_ptr(ptr: *mut AwokenState) -> Self { Self { ptr: AtomicPtr::new(ptr) } }

    fn take(&self, ordering: Ordering) -> Option<ArcAwokenState> {
        match NonNull::new(self.ptr.swap(null_mut(), ordering)) {
            // Safety: We have swapped the ptr with a null. That means we are
            // in charge of managing the pointer's reference count.
            Some(ptr) => Some(unsafe { ArcAwokenState::from_ptr(ptr) }),
            None => None,
        }
    }

    fn swap(&self, new: Option<ArcAwokenState>, ordering: Ordering) -> Option<ArcAwokenState> {
        match new {
            Some(new) => {
                let new_ptr = new.as_ptr();
                mem::forget(new);
                match NonNull::new(self.ptr.swap(new_ptr, ordering)) {
                    // Safety: We have swapped the ptr with another reference
                    // counted ptr. That means we are in charge of managing the
                    // old pointer's reference count.
                    Some(old_ptr) => Some(unsafe { ArcAwokenState::from_ptr(old_ptr) }),
                    None => None,
                }
            },
            None => self.take(ordering),
        }
    }

    fn store(&self, new: Option<ArcAwokenState>, ordering: Ordering) {
        // Like `swap`, but the result is dropped so that the reference counter
        // is decremented if needed.
        let _ = self.swap(new, ordering);
    }

    fn compare_exchange(&self, current: *mut AwokenState, new: Option<ArcAwokenState>, success: Ordering, failure: Ordering) -> Result<Option<ArcAwokenState>, (*mut AwokenState, Option<ArcAwokenState>)> {
        match new {
            Some(new) => {
                let new_ptr = new.as_ptr();
                match self.ptr.compare_exchange(current, new_ptr, success, failure) {
                    // Safety: It is guaranteed that the Ok result is equal to
                    // `current`. So, we will use the value in our local
                    // `current` variable so that the compiler can optimize it
                    // more easily.
                    Ok(_) => {
                        mem::forget(new);
                        match NonNull::new(current) {
                            Some(current) => Ok(Some(unsafe { ArcAwokenState::from_ptr(current) })),
                            None => Ok(None),
                        }
                    },
                    Err(actual) => Err((actual, Some(new))),
                }
            },
            None => {
                match self.ptr.compare_exchange(current, null_mut(), success, failure) {
                    // Safety: It is guaranteed that the Ok result is equal to
                    // `current`. So, we will use the value in our local
                    // `current` variable so that the compiler can optimize it
                    // more easily.
                    Ok(_) => match NonNull::new(current) {
                        Some(current) => Ok(Some(unsafe { ArcAwokenState::from_ptr(current) })),
                        None => Ok(None),
                    },
                    Err(actual) => Err((actual, None)),
                }
            },
        }
    }

    fn compare_exchange_spin_lock(&self, current: *mut AwokenState, mut new: Option<ArcAwokenState>, success: Ordering) -> Option<ArcAwokenState> {
        loop {
            match self.compare_exchange(current, new, success, Ordering::Relaxed) {
                Ok(current) => return current,
                Err((_, still_new)) => new = still_new,
            }
            spin_loop()
        }
    }
}

impl Drop for AtomicArcAwokenState {
    fn drop(&mut self) {
        // If we still point to something, load it and drop it.
        // Since it is an `ArcAwokenState`, the reference count is
        // automatically decremented.
        // Otherwise, this just drops `None` which has no effect.
        drop(self.take(Ordering::Acquire))
    }
}

#[derive(Debug)]
struct NeighborPtr {
    /// The number of nodes actively following this pointer who have not yet updated the reference
    /// count at the node it points to. This will prevent this pointer from being dropped before the
    /// neighbor's reference count can be incremented.
    in_use: AtomicUsize,
    /// A pointer to a neighboring node. Can be null.
    neighbor: AtomicArcAwokenState,
}

impl NeighborPtr {
    /// Creates a struct representing a pointer to a neighbor. The `neighbor` is null and `in_use`
    /// is set to `0`.
    fn new() -> Self {
        Self {
            in_use: AtomicUsize::new(0),
            neighbor: AtomicArcAwokenState::new_null(),
        }
    }

    /// Follows the neighbor pointer safely, incrementing and decrementing reference counts as
    /// needed.
    fn follow(&self) -> Option<ArcAwokenState>{
        self.in_use.fetch_add(1, Ordering::Acquire);
        let arc_ptr = match NonNull::new(self.neighbor.ptr.load(Ordering::Acquire)) {
            Some(neighbor_ptr) => {
                // We have incremented the `in_use` counter. That indicates
                // that whatever is being pointed to cannot be deallocated
                // until that counter is decremented.
                unsafe { neighbor_ptr.ref_count() }.fetch_add(1, Ordering::Acquire);
                let neighbor_ptr = unsafe { ArcAwokenState::from_ptr(neighbor_ptr) };
                Some(neighbor_ptr)
            },
            None => None,
        };
        // Use Relaxed because we have made no changes to the data that the
        // neighbor ptr points to.
        self.in_use.fetch_sub(1, Ordering::Relaxed);
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

    fn take(&self, ordering: Ordering) -> Option<ArcAwokenState> {
        self.neighbor.take(ordering)
    }

    fn swap(&self, new: Option<ArcAwokenState>, ordering: Ordering) -> Option<ArcAwokenState> {
        self.neighbor.swap(new, ordering)
    }

    fn store(&self, new: Option<ArcAwokenState>, ordering: Ordering) {
        self.neighbor.store(new, ordering)
    }

    fn compare_exchange(&self, current: *mut AwokenState, new: Option<ArcAwokenState>, success: Ordering, failure: Ordering) -> Result<Option<ArcAwokenState>, (*mut AwokenState, Option<ArcAwokenState>)> {
        self.neighbor.compare_exchange(current, new, success, failure)
    }

    fn compare_exchange_spin_lock(&self, current: *mut AwokenState, new: Option<ArcAwokenState>, success: Ordering) -> Option<ArcAwokenState> {
        self.neighbor.compare_exchange_spin_lock(current, new, success)
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

        let self_left = self.state.left().swap(None, Ordering::Acquire);
        let self_right = self.state.right().follow();

        match self_left {
            Some(self_left) => {
                if self_left.as_ptr() == self_addr {
                    self.remove_as_awoken();
                } else {
                    self.remove_as_body(awake_token, self_addr, self_left, self_right)
                }
            },
            None => self.remove_as_head(awake_token, self_addr, self_right),
        }
    }

    fn remove_as_awoken(&self) {
        self.state.left().store(None, Ordering::Release);
        self.state.right().store(None, Ordering::Release);
        while self.state.ref_count().load(Ordering::Acquire) > 0 {
            spin_loop();
            self.state.left().store(None, Ordering::Release);
            self.state.right().store(None, Ordering::Release);
        }
        self.state.right().wait();
    }

    fn remove_as_body(&self, awake_token: &AwakeToken, self_addr: *mut AwokenState, self_left: ArcAwokenState, self_right: Option<ArcAwokenState>) {
        // Invariants:
        // - No nodes can every be added before you in the list. They can only
        //   be appended to the tail.
        //   - We can use this to guarantee that if we pass the tail to a
        //     pointer before us in the list, it will not come back to us.
        //   - We can use this to guarantee that if you are the head of the
        //     list, if any nodes exist before you, they are all being removed.

        let mut wait_on_head = false;
        let mut check_tail = true;
        let mut check_head = true;
        let mut check_rights_left = true;

        self_left.right().store(self_right.clone(), Ordering::Release);
        let mut wait_self_left;
        if self_left.right().in_use.load(Ordering::Acquire) == 0 {
            wait_self_left = None;
        } else {
            wait_self_left = Some(self_left.clone());
        }

        match self_right {
            Some(self_right) => {
                if self_right.as_ptr() == self_addr {
                    // We are being woken up while being removed. The waking
                    // function will clobber our left and right pointers.
                    if let Some(self_left) = wait_self_left {
                        self_left.right().wait();
                    }
                    return self.remove_as_awoken();
                }
                if let Err((_, self_left)) = self_right.left().compare_exchange(self_addr, Some(self_left), Ordering::Release, Ordering::Relaxed) {
                    check_rights_left = self_right.left().compare_exchange(null_mut(), self_left, Ordering::Release, Ordering::Relaxed).is_err();
                } else {
                    check_rights_left = false;
                }
            },
            None => {
                // If we become the tail, try to pass it along.
                check_tail = awake_token.wakers.tail.compare_exchange(self_addr, Some(self_left), Ordering::Release, Ordering::Relaxed).is_err();
            },
        }

        while wait_on_head || wait_self_left.is_some() || self.state.ref_count().load(Ordering::Acquire) > 0 {
            let self_left = self.state.left().swap(None, Ordering::Acquire);
            let self_right = self.state.right().follow();
            match (self_left, self_right) {
                (None, self_right) => {
                    if check_head && awake_token.wakers.head.compare_exchange(self_addr, self_right, Ordering::Release, Ordering::Relaxed).is_ok() {
                        wait_on_head = true;
                        check_head = false;
                    }
                    check_tail = check_tail && awake_token.wakers.tail.compare_exchange(self_addr, None, Ordering::Release, Ordering::Relaxed).is_err();
                },
                (Some(self_left), self_right) => {
                    if self_left.as_ptr() == self_addr {
                        // We are being woken up while being removed. The waking
                        // function will clobber our left and right pointers.
                        if let Some(wait_self_left) = wait_self_left {
                            wait_self_left.right().wait();
                        }
                        if wait_on_head {
                            awake_token.wakers.head.wait();
                        }
                        return self.remove_as_awoken();
                    }

                    self_left.right().store(self_right.clone(), Ordering::Release);
                    if self_left.right().in_use.load(Ordering::Acquire) > 0 {
                        // If `wait_self_left` is not being used, we can reuse that variable.
                        // Otherwise, we'll need to wait.
                        if let Some(wait_self_left) = wait_self_left {
                            wait_self_left.right().wait();
                        }
                        wait_self_left = Some(self_left.clone())
                    }

                    match self_right {
                        Some(self_right) => {
                            if self_right.as_ptr() == self_addr {
                                // We are being woken up while being removed. The waking
                                // function will clobber our left and right pointers.
                                if let Some(wait_self_left) = wait_self_left {
                                    wait_self_left.right().wait();
                                }
                                if wait_on_head {
                                    awake_token.wakers.head.wait();
                                }
                                return self.remove_as_awoken();
                            }
                            if check_rights_left {
                                if let Err((_, self_left)) = self_right.left().compare_exchange(self_addr, Some(self_left), Ordering::Release, Ordering::Relaxed) {
                                    check_rights_left = self_right.left().compare_exchange(null_mut(), self_left, Ordering::Release, Ordering::Relaxed).is_err();
                                } else {
                                    check_rights_left = false;
                                }
                            }
                        },
                        None => {
                            // If we become the tail, try to pass it along.
                            check_tail = check_tail && awake_token.wakers.tail.compare_exchange(self_addr, Some(self_left), Ordering::Release, Ordering::Relaxed).is_err();
                        },
                    }
                },
            }

            if let Some(self_left) = &wait_self_left {
                if self_left.right().in_use.load(Ordering::Acquire) == 0 {
                    wait_self_left = None;
                }
            }

            if wait_on_head && (awake_token.wakers.head.in_use.load(Ordering::Acquire) == 0) {
                wait_on_head = false;
            }

            spin_loop();
        }

        // Wait for anybody actively iterating through us to have made it to the
        // next node.
        self.state.right().wait();
        // Clear paths to reach our neighbors.
        self.state.right().store(None, Ordering::Release);
    }

    fn remove_as_head(&self, awake_token: &AwakeToken, self_addr: *mut AwokenState, self_right: Option<ArcAwokenState>) {
        // Invariants:
        // - No nodes can every be added before you in the list. They can only
        //   be appended to the tail.
        //   - We can use this to guarantee that if we pass the tail to a
        //     pointer before us in the list, it will not come back to us.
        //   - We can use this to guarantee that if you are the head of the
        //     list, if any nodes exist before you, they are all being removed.

        let mut check_tail = true;
        let mut check_rights_left = true;

        awake_token.wakers.head.store(self_right.clone(), Ordering::Release);
        let mut wait_on_head = true;

        match self_right {
            Some(self_right) => {
                if self_right.as_ptr() == self_addr {
                    // We are being woken up while being removed. The waking
                    // function will clobber our left and right pointers.
                    return self.remove_as_awoken();
                }

                if let Err((_, self_left)) = self_right.left().compare_exchange(self_addr, None, Ordering::Release, Ordering::Relaxed) {
                    check_rights_left = self_right.left().compare_exchange(null_mut(), self_left, Ordering::Release, Ordering::Relaxed).is_err();
                } else {
                    check_rights_left = false;
                }
            },
            None => {
                // If we become the tail, try to pass it along.
                check_tail = awake_token.wakers.tail.compare_exchange(self_addr, None, Ordering::Release, Ordering::Relaxed).is_err();
            },
        }

        while wait_on_head || self.state.ref_count().load(Ordering::Acquire) > 0 {
            let self_right = self.state.right().follow();
            match self_right {
                None => {
                    check_tail = check_tail && awake_token.wakers.tail.compare_exchange(self_addr, None, Ordering::Release, Ordering::Relaxed).is_err();
                },
                Some(self_right) => {
                    if self_right.as_ptr() == self_addr {
                        // We are being woken up while being removed. The waking
                        // function will clobber our left and right pointers.
                        if wait_on_head {
                            awake_token.wakers.head.wait();
                        }
                        return self.remove_as_awoken();
                    }

                    if check_rights_left {
                        if let Err((_, self_left)) = self_right.left().compare_exchange(self_addr, None, Ordering::Release, Ordering::Relaxed) {
                            check_rights_left = self_right.left().compare_exchange(null_mut(), self_left, Ordering::Release, Ordering::Relaxed).is_err();
                        } else {
                            check_rights_left = false;
                        }
                    }
                },
            }

            if wait_on_head && (awake_token.wakers.head.in_use.load(Ordering::Acquire) == 0) {
                wait_on_head = false;
            }

            spin_loop();
        }

        // Wait for anybody actively iterating through us to have made it to the
        // next node.
        self.state.right().wait();
        // Clear paths to reach our neighbors.
        self.state.right().store(None, Ordering::Release);
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
                        left: AtomicArcAwokenState::new_null(),
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
                    let arc_self1 = unsafe { ArcAwokenState::from_ptr(NonNull::new_unchecked(self_addr)) };
                    let arc_self2 = unsafe { ArcAwokenState::from_ptr(NonNull::new_unchecked(self_addr)) };

                    // We own this pointer to our left neighbor even though it has not yet been
                    // added to our left pointer. Our neighbor's reference count already accounts
                    // for this reference since we took it directly from `tail`.
                    let left_neighbor_ptr = awake_token.wakers.tail.swap(Some(arc_self1), Ordering::AcqRel);
                    match left_neighbor_ptr {
                        // Special case: If the tail was null, that means we are the first node
                        // in the list. Aka. we can also store our address for head. This brings
                        // us to a consistent state early.
                        None => {
                            // Note: Reference count accounted for at start.
                            awake_token.wakers.head.store(Some(arc_self2), Ordering::Release);
                        },
                        // Normal case: There is a neighbor to our left that we need to finish
                        // connecting  ourself to.
                        Some(left_neighbor) => {
                            // Note: Reference counts accounted for at start.
                            let left_neighbor_ptr = left_neighbor.as_non_null();
                            self.state.left().store(Some(left_neighbor), Ordering::Release);
                            // Now need to add ourself as their neighbor.
                            // We just stored our reference to the token to the
                            // left. Only neighbors to our left can modify our
                            // left pointer. So, our reference is safe until
                            // the left neighbors `right` pointer is updated.
                            unsafe { left_neighbor_ptr.right() }.compare_exchange_spin_lock(null_mut(), Some(arc_self2), Ordering::AcqRel);
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
