/// Trait for whether changing a setting requires `sv_cheats 1` to be set.  
/// I'm unsure as to the exact design this will most benefit from, but currently it will be
/// implemented on specific field types
pub trait RequiresCheats {
    fn requires_cheats(&self, to: &Self) -> bool;
}

/// Implements [`RequiresCheats`] on the given types, marking all of them
/// as not requiring cheats to change.
#[macro_export]
macro_rules! cheats_none {
    ($($ty:ty),*) => {
        $(
            impl $crate::conf::cheat::RequiresCheats for $ty {
                fn requires_cheats(&self, _to: &$ty) -> bool {
                    false
                }
            }
        )*
    };
}

/// Implements [`RequiresCheats`] on the given types, marking all of them
/// as requiring cheats to change.
#[macro_export]
macro_rules! cheats_all {
    ($($ty:ty),*) => {
        $(
            impl $crate::conf::cheat::RequiresCheats for $ty {
                fn requires_cheats(&self, _to: &$ty) -> bool {
                    true
                }
            }
        )*
    };
}
