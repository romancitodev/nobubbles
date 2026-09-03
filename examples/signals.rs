use nobubbles::signals::{ReadSignal, WriteSignal, signal};

fn main() {
  let signal = signal(0);
  read_signal(signal);
  write(signal);
  read_signal(signal);
}

fn read_signal<T: std::fmt::Debug + 'static>(signal: impl Into<ReadSignal<T>>) {
  let signal = signal.into();
  let borrow = signal.borrow();
  println!("{:?}", *borrow);
}

fn write(signal: impl Into<WriteSignal<i32>>) {
  let signal = signal.into();
  let mut borrow = signal.borrow_mut();
  *borrow = 2;
}
