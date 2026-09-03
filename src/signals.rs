use std::{
  any::Any,
  cell::{Cell, Ref, RefCell, RefMut},
  marker::PhantomData,
  ops::{Deref, DerefMut},
  rc::Rc,
};

thread_local! {
  pub static RT: Runtime = Runtime::default();
}

type Slot = Rc<RefCell<dyn Any>>;

#[derive(Default)]
pub struct Runtime {
  slots: RefCell<Vec<Slot>>,
  dirty: Cell<bool>,
  quit: Cell<bool>,
}

/// It takes the `slot` from the `Runtime` and returns a clone of it.
/// This is used to get the `slot` from the `Runtime` without having to borrow the `Runtime` for the entire duration of the function.
fn slot(idx: usize) -> Slot {
  RT.with(|ctx| Rc::clone(&ctx.slots.borrow()[idx]))
}

pub fn quit() {
  RT.with(|ctx| ctx.quit.set(true));
}

/// It returns the value of the `quit` flag of the `Runtime`.
#[must_use]
pub fn should_quit() -> bool {
  RT.with(|ctx| ctx.quit.get())
}

/// It sets the `dirty` flag of the `Runtime` to `true`.
fn make_dirty() {
  RT.with(|ctx| ctx.dirty.set(true));
}

/// It returns the value of the `dirty` flag of the `Runtime`.
#[must_use]
pub fn is_dirty() -> bool {
  RT.with(|ctx| ctx.dirty.get())
}

/// It sets the `dirty` flag of the `Runtime` to `false`.
pub fn clear_dirty() {
  RT.with(|ctx| ctx.dirty.set(false));
}

pub fn signal<T: 'static>(value: T) -> Signal<T> {
  RT.with(|ctx| {
    let mut slots = ctx.slots.borrow_mut();
    slots.push(Rc::new(RefCell::new(value)));
    Signal(slots.len() - 1, PhantomData)
  })
}

impl<T: 'static> Signal<T> {
  /// It returns a `SignalRef` that allows you to borrow the value of the signal.
  ///
  /// # Panics
  /// If a guard for the same signal is already live. Different signals never conflict.
  #[must_use]
  pub fn borrow(self) -> SignalRef<T> {
    self.read().borrow()
  }

  /// It returns a `SignalRefMut` that allows you to borrow the mutable value of the signal.
  ///
  /// # Panics
  /// If a guard for the same signal is already live. different signals never conflict.
  #[must_use]
  pub fn borrow_mut(self) -> SignalRefMut<T> {
    self.write().borrow_mut()
  }

  /// It takes a closure `f` and applies it to the value of the signal, returning the result.
  pub fn with_ref<R>(self, f: impl FnOnce(&T) -> R) -> R {
    self.read().with_ref(f)
  }

  /// Applies `f` to a mutable reference to the value, returning the result.
  /// Marks the runtime dirty.
  pub fn update<R>(self, f: impl FnOnce(&mut T) -> R) -> R {
    self.write().update(f)
  }

  pub fn set(self, value: T) {
    self.write().set(value);
  }

  /// It splits the `Signal` into a `ReadSignal` and a `WriteSignal`.
  ///
  /// This can be useful if you want to pass the `ReadSignal` to a component that only needs to read the value, and the `WriteSignal` to a component that only needs to write the value.
  #[must_use]
  pub fn split(self) -> (ReadSignal<T>, WriteSignal<T>) {
    (
      ReadSignal(self.0, PhantomData),
      WriteSignal(self.0, PhantomData),
    )
  }

  /// It converts the `Signal` into a `ReadSignal`.
  ///
  /// This can be useful if you want to pass the `ReadSignal` to a component that only needs to read the value.
  #[must_use]
  pub fn read(self) -> ReadSignal<T> {
    ReadSignal(self.0, PhantomData)
  }

  /// It converts the `Signal` into a `WriteSignal`.
  ///
  /// This can be useful if you want to pass the `WriteSignal` to a component that only needs to write the value.
  #[must_use]
  pub fn write(self) -> WriteSignal<T> {
    WriteSignal(self.0, PhantomData)
  }
}

impl<T: Clone + 'static> Signal<T> {
  /// It returns a clone of the value of the signal.
  ///
  /// # Caveats
  ///
  /// Only use it if `T` it's cheap to clone, otherwise use [`Signal::with_ref`] to avoid unnecessary clones.
  pub fn get(self) -> T {
    self.with_ref(Clone::clone)
  }
}

pub struct Signal<T>(usize, PhantomData<*const T>);
pub struct ReadSignal<T>(usize, PhantomData<*const T>);
pub struct WriteSignal<T>(usize, PhantomData<*const T>);

impl<T: 'static> ReadSignal<T> {
  /// It returns a `SignalRef` that allows you to borrow the value of the signal.
  ///
  /// # Panics
  /// If a guard for the same signal is already live. Different signals never conflict.
  #[must_use]
  pub fn borrow(self) -> SignalRef<T> {
    let slot = slot(self.0);
    let value = Ref::map(slot.borrow(), |any| {
      any
        .downcast_ref::<T>()
        .expect("Signal value is of the wrong type")
    });
    // SAFETY: the `Ref` points inside the alloc owned by `slot`, which this
    // guard keeps alive for as long as it exists. The invariant that makes this
    // sound is the *field order* in `SignalRef`. `value` is dropped before `_slot`.
    let value: Ref<'_, T> = unsafe { std::mem::transmute(value) };
    SignalRef { value, _slot: slot }
  }

  pub fn with_ref<R>(self, f: impl FnOnce(&T) -> R) -> R {
    f(&self.borrow())
  }
}

impl<T: 'static> WriteSignal<T> {
  /// It returns a `SignalRefMut` that allows you to borrow the mutable value of the signal.
  ///
  /// # Panics
  /// If a guard for the same signal is already live. Different signals never conflict.
  #[must_use]
  pub fn borrow_mut(self) -> SignalRefMut<T> {
    let slot = slot(self.0);
    let value = RefMut::map(slot.borrow_mut(), |any| {
      any
        .downcast_mut::<T>()
        .expect("Signal value is of the wrong type")
    });
    // SAFETY: the `Ref` points inside the alloc owned by `slot`, which this
    // guard keeps alive for as long as it exists. The invariant that makes this
    // sound is the *field order* in `SignalRef`. `value` is dropped before `_slot`.
    let value: RefMut<'_, T> = unsafe { std::mem::transmute(value) };
    SignalRefMut { value, _slot: slot }
  }

  /// It takes a closure `f` and applies it to the mutable value of the signal, returning the result.
  pub fn update<R>(self, f: impl FnOnce(&mut T) -> R) -> R {
    f(&mut self.borrow_mut())
  }

  /// It takes a value of type `T` and sets the value of the signal to that value.
  pub fn set(self, value: T) {
    *self.borrow_mut() = value;
  }
}

pub struct SignalRef<T: 'static> {
  /// SAFETY: the fields order in this struct is important, because of the `drop` behaviour.
  value: Ref<'static, T>,
  _slot: Slot,
}

pub struct SignalRefMut<T: 'static> {
  /// SAFETY: the fields order in this struct is important, because of the `drop` behaviour.
  value: RefMut<'static, T>,
  _slot: Slot,
}

impl<T: 'static> Deref for SignalRefMut<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.value
  }
}

impl<T: 'static> DerefMut for SignalRefMut<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.value
  }
}

/// When a `SignalRefMut` is dropped, it marks the signal as dirty by calling `make_dirty()`. This is important because it indicates that the value of the signal has been modified, and any dependent computations should be re-evaluated.
impl<T: 'static> Drop for SignalRefMut<T> {
  fn drop(&mut self) {
    make_dirty();
  }
}

impl<T: 'static> Deref for SignalRef<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.value
  }
}

macro_rules! copy_handle {
  ($($t:ident),*) => { $(
  impl<T> Clone for $t<T> {
    fn clone(&self) -> Self { *self }
  }

  impl<T> Copy for $t<T> {}
  )*};
}

copy_handle!(Signal, ReadSignal, WriteSignal);

/// This is a convenience implementation that allows you to convert a `Signal` into a `ReadSignal` using the `From` trait
/// so the components can accept an `impl Into<ReadSignal<T>>`.
impl<T> From<Signal<T>> for ReadSignal<T> {
  fn from(signal: Signal<T>) -> Self {
    ReadSignal(signal.0, PhantomData)
  }
}

impl<T> From<Signal<T>> for WriteSignal<T> {
  fn from(signal: Signal<T>) -> Self {
    WriteSignal(signal.0, PhantomData)
  }
}

macro_rules! display_handle {
  ($($t:ident),*) => { $(
  impl<T: std::fmt::Display + 'static> std::fmt::Display for $t<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      self.with_ref(|v| write!(f, "{v}"))
    }
  }
  )*};
}

display_handle!(Signal, ReadSignal);

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn get_set() {
    let s = signal(0);
    assert_eq!(s.get(), 0);
    s.set(1);
    assert_eq!(s.get(), 1);
  }

  #[test]
  fn independent_signals() {
    let s1 = signal(0);
    let s2 = signal(0);
    let arr = signal(vec![1, 2, 3]);
    s1.set(1);
    assert_eq!(s1.get(), 1);
    assert_eq!(s2.get(), 0);
    assert_eq!(arr.with_ref(Vec::len), 3);
  }

  #[test]
  fn read_do_not_dirty() {
    let s = signal(0);
    clear_dirty();
    let _ = s.get();
    s.with_ref(|v| *v);
    assert!(!is_dirty());
  }

  #[test]
  fn write_dirty() {
    let s = signal(0);
    clear_dirty();
    s.set(1);
    assert!(is_dirty());
  }

  #[test]
  fn signals_can_update_others() {
    let price = signal(5);
    let total = signal(10);
    total.update(|t| *t += price.get());
    assert_eq!(*total.read().borrow(), 15);
  }

  #[test]
  fn sugar_syntax() {
    let (sr, wr) = signal(0).split();
    wr.set(1);
    assert_eq!(*sr.borrow(), 1);
  }

  #[test]
  fn strings_are_copy_now_lol() {
    let s = signal(String::from("hello"));
    let a = s;
    let b = s;
    assert_eq!(a.get(), b.get());
  }

  #[test]
  #[should_panic = "already mutably borrowed"]
  fn self_referencial_update_panics() {
    let s = signal(1);
    s.update(|v| *v += s.get());
  }

  #[test]
  fn quit_the_app() {
    quit();
    assert!(should_quit());
  }
}
