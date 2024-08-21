use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Test;

#[wasm_bindgen]
impl Test {
    pub fn hello() -> String {
        String::from("Hello, world!")
    }
}
