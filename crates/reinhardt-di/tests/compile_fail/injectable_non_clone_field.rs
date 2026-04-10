// Tests that auto-derived Clone fails for types with non-Clone fields.
// The struct is Send + Sync + Default, so only the Clone error remains.
use reinhardt_di_macros::injectable;

// A type that is Send + Sync + Default but NOT Clone
struct NonCloneValue(std::sync::Mutex<String>);

impl Default for NonCloneValue {
    fn default() -> Self {
        Self(std::sync::Mutex::new(String::new()))
    }
}

// Safety: Mutex<T> is already Send + Sync, so the wrapper is too.

#[injectable]
#[derive(Default)]
struct ServiceWithNonCloneField {
    #[no_inject]
    value: NonCloneValue,
}

fn main() {}
