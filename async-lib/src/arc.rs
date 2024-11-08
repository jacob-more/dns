use std::{hint::spin_loop, mem::ManuallyDrop, ptr::{addr_of_mut, null_mut, NonNull}, sync::atomic::{AtomicPtr, AtomicUsize, Ordering}};

pub trait ReferenceCounted {
    fn ref_count(&self) -> &AtomicUsize;
}

pub struct ArcPtr<T> where T: ReferenceCounted {
    ptr: NonNull<T>
}

impl<T> ArcPtr<T> where T: ReferenceCounted {
    pub fn new(value: &mut T) -> Self {
        value.ref_count().fetch_add(1, Ordering::AcqRel);
        match NonNull::new(addr_of_mut!(*value)) {
            Some(ptr) => Self { ptr },
            None => panic!("The reference was null"),
        }
    }

    /// The pointer must be associated with a reference count if it is non-null. Calling this with a
    /// non-null pointer passes the responsibility of decrementing the reference count to the
    /// AtomicArcPtr.
    pub unsafe fn from_ptr(ptr: NonNull<T>) -> Self {
        Self { ptr }
    }

    pub fn as_non_null(&self) -> NonNull<T> {
        self.ptr
    }

    pub fn as_ptr(&self) -> *mut T {
        self.ptr.as_ptr()
    }

    pub fn as_ref(&self) -> &T {
        unsafe { self.ptr.as_ref() }
    }

    pub fn as_mut(&mut self) -> &mut T {
        unsafe { self.ptr.as_mut() }
    }

    pub fn ref_count(&self) -> &AtomicUsize {
        self.as_ref().ref_count()
    }
}

impl<T> Clone for ArcPtr<T> where T: ReferenceCounted {
    fn clone(&self) -> Self {
        self.ref_count().fetch_add(1, Ordering::Release);
        Self { ptr: self.ptr.clone() }
    }
}

impl<T> Drop for ArcPtr<T> where T: ReferenceCounted {
    fn drop(&mut self) {
        self.ref_count().fetch_sub(1, Ordering::Release);
    }
}

struct AtomicArcPtr<T> where T: ReferenceCounted {
    ptr: AtomicPtr<T>
}

impl<T> AtomicArcPtr<T> where T: ReferenceCounted {
    pub fn new_null() -> Self {
        Self { ptr: AtomicPtr::new(null_mut()) }
    }

    pub fn new(value: ArcPtr<T>) -> Self {
        let value = ManuallyDrop::new(value);
        Self { ptr: AtomicPtr::new(value.as_ptr()) }
    }

    /// The pointer must be associated with a reference count if it is non-null. Calling this with a
    /// non-null pointer passes the responsibility of decrementing the reference count to the
    /// AtomicArcPtr.
    pub unsafe fn from_ptr(ptr: NonNull<T>) -> Self {
        Self { ptr: AtomicPtr::new(ptr.as_ptr()) }
    }

    /// There is no guarantee that the loaded pointer does not point to a dropped value unless an
    /// ArcPtr of that value is already held for the returned pointer.
    pub unsafe fn load(&self, ordering: Ordering) -> Option<ArcPtr<T>> {
        match NonNull::new(self.ptr.load(ordering)) {
            Some(ptr) => Some(ArcPtr::from_ptr(ptr)),
            None => None,
        }
    }

    pub fn take(&self, ordering: Ordering) -> Option<ArcPtr<T>> {
        self.swap(None, ordering)
    }

    pub fn swap(&self, new: Option<ArcPtr<T>>, ordering: Ordering) -> Option<ArcPtr<T>> {
        match new {
            Some(new) => {
                let new = ManuallyDrop::new(new);
                match NonNull::new(self.ptr.swap(new.as_ptr(), ordering)) {
                    // Safety: We have swapped the ptr with another reference counted ptr. That
                    // means we are in charge of managing the old pointer's reference count and the
                    // reference count we were previously in charge of is now managed by this
                    // AtomicArcPtr.
                    Some(old_ptr) => Some(unsafe { ArcPtr::from_ptr(old_ptr) }),
                    None => None,
                }
            },
            None => self.take(ordering),
        }
    }

    pub fn store(&self, new: Option<ArcPtr<T>>, ordering: Ordering) {
        // Like `swap`, but the result is dropped so that the reference counter is decremented if
        // needed.
        let _ = self.swap(new, ordering);
    }

    pub fn compare_exchange(&self, current: *mut T, new: Option<ArcPtr<T>>, success: Ordering, failure: Ordering) -> Result<Option<ArcPtr<T>>, (*mut T, Option<ArcPtr<T>>)> {
        match new {
            Some(new) => {
                let new_ptr = new.as_ptr();
                match self.ptr.compare_exchange(current, new_ptr, success, failure) {
                    // Safety: It is guaranteed that the Ok result is equal to
                    // `current`. So, we will use the value in our local
                    // `current` variable so that the compiler can optimize it
                    // more easily.
                    Ok(_) => {
                        ManuallyDrop::new(new);
                        match NonNull::new(current) {
                            Some(current) => Ok(Some(unsafe { ArcPtr::from_ptr(current) })),
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
                        Some(current) => Ok(Some(unsafe { ArcPtr::from_ptr(current) })),
                        None => Ok(None),
                    },
                    Err(actual) => Err((actual, None)),
                }
            },
        }
    }
}

impl<T> Drop for AtomicArcPtr<T> where T: ReferenceCounted {
    fn drop(&mut self) {
        // If we still point to something, load it and drop it. Since it is an `ArcPtr`, the
        // reference count is automatically decremented.
        // Otherwise, this just drops `None` which has no effect.
        let arc_ptr = self.take(Ordering::Acquire);
        drop(arc_ptr);
    }
}

/// An atomically reference counted pointer that cannot be read unless it is owned (i.e., replaced
/// with some other value) or take (replaced with a null value).
pub struct AtomicArcSwapPtr<T> where T: ReferenceCounted {
    arc: AtomicArcPtr<T>
}

impl<T> AtomicArcSwapPtr<T> where T: ReferenceCounted {
    pub fn new_null() -> Self {
        Self { arc: AtomicArcPtr::new_null() }
    }

    pub fn new(value: ArcPtr<T>) -> Self {
        Self { arc: AtomicArcPtr::new(value) }
    }

    /// The pointer must be associated with a reference count if it is non-null. Calling this with a
    /// non-null pointer passes the responsibility of decrementing the reference count to the
    /// AtomicArcPtr.
    pub unsafe fn from_ptr(ptr: NonNull<T>) -> Self {
        Self { arc: AtomicArcPtr::from_ptr(ptr) }
    }

    pub fn take(&self, ordering: Ordering) -> Option<ArcPtr<T>> {
        self.arc.take(ordering)
    }

    pub fn swap(&self, new: Option<ArcPtr<T>>, ordering: Ordering) -> Option<ArcPtr<T>> {
        self.arc.swap(new, ordering)
    }

    pub fn store(&self, new: Option<ArcPtr<T>>, ordering: Ordering) {
        self.arc.store(new, ordering)
    }

    pub fn compare_exchange(&self, current: *mut T, new: Option<ArcPtr<T>>, success: Ordering, failure: Ordering) -> Result<Option<ArcPtr<T>>, (*mut T, Option<ArcPtr<T>>)> {
        self.arc.compare_exchange(current, new, success, failure)
    }
}

pub struct ArcFollowPtr<'a, T> where T: ReferenceCounted {
    atomic_arc_follow: &'a AtomicArcFollowPtr<T>,
    arc: ArcPtr<T>,
}

impl<'a, T> ArcFollowPtr<'a, T> where T: ReferenceCounted {
    pub fn as_non_null(&self) -> NonNull<T> {
        self.arc.as_non_null()
    }

    pub fn as_ptr(&self) -> *mut T {
        self.arc.as_ptr()
    }

    pub fn as_ref(&self) -> &T {
        self.arc.as_ref()
    }

    pub fn as_mut(&mut self) -> &mut T {
        self.arc.as_mut()
    }

    pub fn ref_count(&self) -> &AtomicUsize {
        self.as_ref().ref_count()
    }

    pub fn try_into_arc_ptr(self) -> Result<ArcPtr<T>, Self> {
        if self.atomic_arc_follow.is_followed.load(Ordering::Acquire) == 0 {
            let this = ManuallyDrop::new(self);
            Ok(unsafe { ArcPtr::from_ptr(this.arc.ptr.clone()) })
        } else {
            Err(self)
        }
    }

    pub fn into_arc_ptr(self) -> ArcPtr<T> {
        while self.atomic_arc_follow.is_followed.load(Ordering::Acquire) > 0 {
            spin_loop();
        }
        let this = ManuallyDrop::new(self);
        unsafe { ArcPtr::from_ptr(this.arc.ptr.clone()) }
    }
}

impl<'a, T> Drop for ArcFollowPtr<'a, T> where T: ReferenceCounted {
    fn drop(&mut self) {
        while self.atomic_arc_follow.is_followed.load(Ordering::Acquire) > 0 {
            spin_loop();
        }
        self.ref_count().fetch_sub(1, Ordering::Release);
    }
}

/// An atomically reference counted pointer that can be loaded if done carefully.
pub struct AtomicArcFollowPtr<T> where T: ReferenceCounted {
    arc: AtomicArcPtr<T>,
    is_followed: AtomicUsize,
}

impl<T> AtomicArcFollowPtr<T> where T: ReferenceCounted {
    pub fn new_null() -> Self {
        Self {
            arc: AtomicArcPtr::new_null(),
            is_followed: AtomicUsize::new(0)
        }
    }

    pub fn new(value: ArcPtr<T>) -> Self {
        Self {
            arc: AtomicArcPtr::new(value),
            is_followed: AtomicUsize::new(0)
        }
    }

    /// The pointer must be associated with a reference count if it is non-null. Calling this with a
    /// non-null pointer passes the responsibility of decrementing the reference count to the
    /// AtomicArcPtr.
    pub unsafe fn from_ptr(ptr: NonNull<T>) -> Self {
        Self {
            arc: AtomicArcPtr::from_ptr(ptr),
            is_followed: AtomicUsize::new(0)
        }
    }

    pub fn load(&self) -> Option<ArcPtr<T>> {
        self.is_followed.fetch_add(1, Ordering::Acquire);
        let arc_ptr = unsafe { self.arc.load(Ordering::Acquire) };
        self.is_followed.fetch_sub(1, Ordering::Release);
        arc_ptr
    }

    pub fn take<'a>(&'a self, ordering: Ordering) -> Option<ArcFollowPtr<'a, T>> {
        match self.arc.take(ordering) {
            Some(arc) => Some(ArcFollowPtr { atomic_arc_follow: &self, arc }),
            None => None,
        }
    }

    pub fn swap<'a>(&'a self, new: Option<ArcPtr<T>>, ordering: Ordering) -> Option<ArcFollowPtr<'a, T>> {
        match self.arc.swap(new, ordering) {
            Some(arc) => Some(ArcFollowPtr { atomic_arc_follow: &self, arc }),
            None => None,
        }
    }

    /// Storing a value without waiting on the follow count can result in the value being pointed to
    /// getting dropped while it is still in use. It is safer to use other functions that return an
    /// `ArcFollowPtr`. These guarantee that the pointer is not in use before dropping their
    /// reference.
    pub unsafe fn store(&self, new: Option<ArcPtr<T>>, ordering: Ordering) {
        self.arc.store(new, ordering)
    }

    pub fn compare_exchange<'a>(&'a self, current: *mut T, new: Option<ArcPtr<T>>, success: Ordering, failure: Ordering) -> Result<Option<ArcFollowPtr<'a, T>>, (*mut T, Option<ArcPtr<T>>)> {
        match self.arc.compare_exchange(current, new, success, failure) {
            Ok(Some(previous_ptr)) => Ok(Some(ArcFollowPtr { atomic_arc_follow: &self, arc: previous_ptr })),
            Ok(None) => Ok(None),
            Err((current, new)) => Err((current, new)),
        }
    }
}
