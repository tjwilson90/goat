pub trait Slot<T>: Default {
    fn get(&self) -> Option<&T>;
    fn set(&mut self, value: T);
}

impl<T> Slot<T> for () {
    fn get(&self) -> Option<&T> {
        None
    }

    fn set(&mut self, _: T) {}
}

impl<T> Slot<T> for Option<T> {
    fn get(&self) -> Option<&T> {
        self.as_ref()
    }

    fn set(&mut self, value: T) {
        *self = Some(value);
    }
}
