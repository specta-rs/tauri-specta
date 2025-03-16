// TODO: Fix the issue causing this to be broken out

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Serialize, Deserialize, Type)]
pub struct MyClass {
    id: i32,
}

#[tauri_specta::class]
impl MyClass {
    pub fn method1(self, some_arg: String) {
        println!("METHOD1: {:?} {}", self, some_arg);
    }

    pub fn method2(some_arg: String) {
        println!("METHOD2: {}", some_arg);
    }

    // TODO: Support these
    // pub fn method3(&mut self, some_arg: String) {
    //     println!("METHOD3: {:?} {}", self, some_arg);
    // }
    // pub fn method4(&self, some_arg: String) {
    //     println!("METHOD4: {:?} {}", self, some_arg);
    // }
}
