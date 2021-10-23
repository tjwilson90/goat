use std::collections::HashMap;

use crate::UserId;

pub trait UserDb {
    fn insert(&mut self, user_id: UserId, name: String);
    fn remove(&mut self, user_id: &UserId);
}

impl UserDb for HashMap<UserId, String> {
    fn insert(&mut self, user_id: UserId, name: String) {
        self.insert(user_id, name);
    }

    fn remove(&mut self, user_id: &UserId) {
        self.remove(user_id);
    }
}

impl UserDb for () {
    fn insert(&mut self, _: UserId, _: String) {}
    fn remove(&mut self, _: &UserId) {}
}
