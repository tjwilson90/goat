use std::collections::HashMap;

use crate::{User, UserId};

pub trait UserDb {
    fn insert(&mut self, user_id: UserId, user: User);
    fn remove(&mut self, user_id: &UserId);
}

impl UserDb for HashMap<UserId, User> {
    fn insert(&mut self, user_id: UserId, user: User) {
        self.insert(user_id, user);
    }

    fn remove(&mut self, user_id: &UserId) {
        self.remove(user_id);
    }
}

impl UserDb for () {
    fn insert(&mut self, _: UserId, _: User) {}
    fn remove(&mut self, _: &UserId) {}
}
