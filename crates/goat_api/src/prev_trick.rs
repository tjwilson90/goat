use crate::WarTrick;

pub trait PreviousTrick {
    fn empty() -> Self;
    fn set(&mut self, trick: WarTrick);
}

impl PreviousTrick for () {
    fn empty() -> Self {
        ()
    }

    fn set(&mut self, _: WarTrick) {}
}

impl PreviousTrick for Option<WarTrick> {
    fn empty() -> Self {
        None
    }

    fn set(&mut self, trick: WarTrick) {
        *self = Some(trick)
    }
}
