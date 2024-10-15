fn wasm_bindgen(items: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match wasm_bindgen_macro_support::expand(proc_macro2::TokenStream::new(), items.into()) {
        Ok(tokens) => tokens.into(),
        Err(diagnostic) => (quote::quote! { #diagnostic }).into(),
    }
}

fn get_impl(attr: proc_macro::TokenStream, name: proc_macro::Ident) -> proc_macro::TokenStream {
    let mut attributes = [
        proc_macro::TokenStream::new(),
        proc_macro::TokenStream::new(),
        proc_macro::TokenStream::new(),
    ];

    let mut idx = 0;

    let mut skip = 0;

    for token in attr {
        if skip > 0 { skip -= 1; continue; }

        match token {
            proc_macro::TokenTree::Ident(ident) => //
                attributes[idx].extend(Some(proc_macro::TokenTree::Ident(ident))),
            proc_macro::TokenTree::Punct(punct) => {
                if idx < 2 {
                    match punct.as_char() {
                        ',' => idx += 1,
                        '=' => {
                            idx += 1;
                            skip += 1;
                        }, _ => attributes[idx].extend({
                            Some(proc_macro::TokenTree::Punct(punct))
                        }),
                    }
                } else {
                    attributes[idx].extend({
                        Some(proc_macro::TokenTree::Punct(punct))
                    })
                }
            },
            token if idx == 2 => attributes[2].extend(Some(token)),
            _ => panic!(),
        }
    }

    let [ty_app, ty_cfg, cfg] = attributes;

    let app_ty = Into::<proc_macro2::TokenStream>::into(ty_app);
    let cfg_ty = Into::<proc_macro2::TokenStream>::into(ty_cfg);
    
    let cfg = Into::<proc_macro2::TokenStream>::into(cfg);

    let name = proc_macro2::Ident::new(
        name.to_string().as_str(), 
        name.span().into(),
    );

    let name = proc_macro2::TokenTree::Ident(name);

    quote::quote! {
        impl #name {
            #[no_mangle]
            pub async fn run() -> Result<(), String> {
                backend::start::<#cfg_ty, #app_ty>(#cfg).await
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

fn get_main(name: proc_macro::Ident) -> proc_macro::TokenStream {
    let name = proc_macro2::Ident::new(name.to_string().as_str(), name.span().into());

    quote::quote! {
        fn main() -> Result<(), String> {
            backend::native::pollster::block_on(#name::run())
        } 
    }.into()
}

#[proc_macro_attribute]
pub fn init_native(
    attr: proc_macro::TokenStream,
    mut decl: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let name = get_name_from_decl(decl.clone());

    decl.extend(get_impl(attr, name.clone()));
    decl.extend(get_main(name));
    decl
}

#[proc_macro_attribute]
pub fn init_wasm32(
    attr: proc_macro::TokenStream,
    decl: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let name = get_name_from_decl(decl.clone());

    let mut items: proc_macro::TokenStream = quote::quote! {
        use backend::web::wasm_bindgen;
        use backend::web::wasm_bindgen_futures;
    }.into();

    items.extend(wasm_bindgen(decl));
    items.extend(wasm_bindgen(get_impl(attr, name)));
    items
}

#[proc_macro]
pub fn init(attr: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let attr = Into::<proc_macro2::TokenStream>::into(attr);

    quote::quote! {
        #[cfg_attr(not(target_arch = "wasm32"), backend::init_native(#attr))]
        #[cfg_attr(target_arch = "wasm32", backend::init_wasm32(#attr))]
        pub struct Wrapper;
    }.into()
}

#[proc_macro]
pub fn run(items: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let items: proc_macro2::TokenStream = items.into();
    
    quote::quote! {
        backend::native::pollster::block_on(#items::run())
    }.into()
}