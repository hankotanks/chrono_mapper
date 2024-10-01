use std::{fmt, error};

#[derive(Debug)]
pub struct WebError { 
    op: &'static str, 
}

impl WebError {
    pub const fn new(op: &'static str) -> Self {
        Self { op }
    }
}

impl fmt::Display for WebError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to {}", self.op)
    }
}

impl error::Error for WebError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> { 
        None 
    }

    fn cause(&self) -> Option<&dyn error::Error> { 
        self.source() 
    }
}

pub fn url() -> Result<String, WebError> {
    web_sys::window()
        .ok_or(WebError::new("obtain window"))?
        .location()
        .href()
        .map_err(|_| WebError::new("query website's base url"))
}

#[cfg(target_arch = "wasm32")]
pub async fn req(
    proxy: winit::event_loop::EventLoopProxy<Vec<u8>>,
    url: &str,
) -> Result<(), WebError> {
    use wasm_bindgen::JsCast as _;

    let opts = web_sys::RequestInit::new();
        opts.set_method("GET");
        opts.set_mode(web_sys::RequestMode::Cors);

    let request = web_sys::Request::new_with_str_and_init(&url, &opts)
        .map_err(|_| WebError::new("initialize request"))?;

    let window = web_sys::window()
        .ok_or(WebError::new("obtain window"))?;

    let resp = window.fetch_with_request(&request);
    let resp = wasm_bindgen_futures::JsFuture::from(resp)
        .await
        .map_err(|_| WebError::new("fetch data"))?
        .dyn_into::<web_sys::Response>()
        .map_err(|_| WebError::new("cast response"))?
        .text()
        .map_err(|_| WebError::new("get response body"))?;

    let bytes = wasm_bindgen_futures::JsFuture::from(resp)
        .await
        .map_err(|_| WebError::new("get response body"))?
        .as_string()
        .unwrap()
        .into_bytes();

    proxy
        .send_event(bytes)
        .map_err(|_| WebError::new("serve data to event loop"))
}