fn wasm_bindgen(items: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match wasm_bindgen_macro_support::expand(proc_macro2::TokenStream::new(), items.into()) {
        Ok(tokens) => tokens.into(),
        Err(diagnostic) => (quote::quote! { #diagnostic }).into(),
    }
}

fn get_impl(attr: proc_macro::TokenStream, name: proc_macro::Ident) -> proc_macro::TokenStream {
    let mut attributes = [
        proc_macro2::TokenStream::new(),
        proc_macro2::TokenStream::new(),
        proc_macro2::TokenStream::new(),
    ];

    let mut idx = 0;

    let mut skip = 0;

    for token in attr {
        if skip > 0 {
            skip -= 1; continue;
        }

        match token {
            proc_macro::TokenTree::Group(_) => panic!(),
            proc_macro::TokenTree::Ident(ident) => {
                let ident: proc_macro2::Ident = proc_macro2::Ident::new(
                    ident.to_string().as_str(), 
                    ident.span().into(),
                );

                let ident = proc_macro2::TokenTree::Ident(ident);

                attributes[idx].extend(Some(ident));
            },
            proc_macro::TokenTree::Punct(punct) => match punct.as_char() {
                ',' => idx += 1,
                '=' => {
                    idx += 1;
                    skip += 1;
                },
                ch => {
                    let spacing = match punct.spacing() {
                        proc_macro::Spacing::Joint => proc_macro2::Spacing::Joint,
                        proc_macro::Spacing::Alone => proc_macro2::Spacing::Alone,
                    };

                    let punct = proc_macro2::Punct::new(ch, spacing);
                    let punct = proc_macro2::TokenTree::Punct(punct);

                    attributes[idx].extend(Some(punct))
                },
            },
            proc_macro::TokenTree::Literal(_) => panic!(),
        }
    }

    let [ty_app, ty_cfg, config] = attributes;

    let name = proc_macro2::Ident::new(
        name.to_string().as_str(), 
        name.span().into(),
    );

    let name = proc_macro2::TokenTree::Ident(name);

    quote::quote! {
        impl #name {
            #[no_mangle]
            #[cfg(target_arch = "wasm32")]
            pub fn set_screen_resolution(
                w: wasm_bindgen::JsValue, 
                h: wasm_bindgen::JsValue,
            ) -> Result<(), String> { 
                backend::set_screen_resolution(w, h)
            }

            #[no_mangle]
            pub async fn run() -> Result<(), String> {
                backend::start::<#ty_cfg, #ty_app>(#config).await
            }
        }
    }.into()
}

fn get_name_from_decl(decl: proc_macro::TokenStream) -> proc_macro::Ident {
    let mut name = None;

    for token in decl.clone() {
        if let proc_macro::TokenTree::Ident(ident) = token {
            let ident_str = ident.to_string();

            if ["pub", "struct", "enum"].into_iter().all(|i| ident_str.ne(i)) {
                name = Some(ident); break;
            }
            
        }
    };

    name.unwrap()
}

#[proc_macro_attribute]
pub fn init_native(
    attr: proc_macro::TokenStream, 
    mut items: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let name = get_name_from_decl(items.clone());

    items.extend(get_impl(attr, name));
    items
}

#[proc_macro_attribute]
pub fn init_wasm32(
    attr: proc_macro::TokenStream, 
    decl: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let name = get_name_from_decl(decl.clone());

    let mut items = proc_macro::TokenStream::new();

    items.extend(wasm_bindgen(decl));
    items.extend(wasm_bindgen(get_impl(attr, name)));
    items
}

#[proc_macro_attribute]
pub fn init(
    attr: proc_macro::TokenStream, 
    decl: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let attr: proc_macro2::TokenStream = attr.into();
    let decl: proc_macro2::TokenStream = decl.into();

    quote::quote! {
        #[cfg_attr(not(target_arch = "wasm32"), backend::init_native(#attr))]
        #[cfg_attr(target_arch = "wasm32", backend::init_wasm32(#attr))]
        #decl
    }.into()
}

#[proc_macro]
pub fn run(items: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let items: proc_macro2::TokenStream = items.into();
    
    quote::quote! {
        backend::native::pollster::block_on(#items::run())
    }.into()
}