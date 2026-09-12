use syn::parse::{Parse, ParseStream};
use syn::{Attribute, Ident, LitStr, Result, Token};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteropMode {
    Command,
    Query,
    Watcher { is_raf: bool },
}

#[derive(Debug, Clone)]
pub struct ItemSpec {
    pub mode: Option<InteropMode>,
    pub original_name: String,
    pub rename_as: Option<Ident>,
}

#[derive(Debug, Clone)]
pub struct BindJsInput {
    pub file_path: LitStr,
    pub wildcard: bool,
    pub items: Vec<ItemSpec>,
}

impl Parse for BindJsInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let file_path: LitStr = input.parse()?;
        input.parse::<Token![::]>()?;

        if input.peek(Token![*]) {
            input.parse::<Token![*]>()?;
            let _ = input.parse::<Option<Token![;]>>()?;
            return Ok(Self {
                file_path,
                wildcard: true,
                items: Vec::new(),
            });
        }

        let content;
        syn::braced!(content in input);

        let mut wildcard = false;
        let mut items = Vec::new();

        while !content.is_empty() {
            if content.peek(Token![*]) {
                content.parse::<Token![*]>()?;
                wildcard = true;
                let _ = content.parse::<Option<Token![,]>>()?;
                continue;
            }

            let mut mode = None;
            if content.peek(Token![#]) {
                let parsed_attrs = content.call(Attribute::parse_outer)?;
                for a in parsed_attrs {
                    if a.path().is_ident("command") {
                        mode = Some(InteropMode::Command);
                    } else if a.path().is_ident("query") {
                        mode = Some(InteropMode::Query);
                    } else if a.path().is_ident("watcher") || a.path().is_ident("monitor") {
                        let mut is_raf = false;
                        if let syn::Meta::List(meta_list) = &a.meta {
                            meta_list.parse_nested_meta(|nested| {
                                if nested.path.is_ident("raf") {
                                    is_raf = true;
                                    Ok(())
                                } else {
                                    Err(nested.error("Unknown watcher attribute argument. Allowed: #[watcher(raf)]"))
                                }
                            })?;
                        }
                        mode = Some(InteropMode::Watcher { is_raf });
                    } else {
                        return Err(syn::Error::new_spanned(
                            a,
                            "Unknown attribute for bind_js item. Allowed: #[command], #[query], #[watcher], #[watcher(raf)]",
                        ));
                    }
                }
            }

            let _ = content.parse::<Option<Token![fn]>>()?;
            let name_ident: Ident = content.parse()?;
            let original_name = name_ident.to_string();

            let mut rename_as = None;
            if content.peek(Token![as]) {
                content.parse::<Token![as]>()?;
                rename_as = Some(content.parse::<Ident>()?);
            }

            items.push(ItemSpec {
                mode,
                original_name,
                rename_as,
            });

            let _ = content.parse::<Option<Token![,]>>()?;
        }

        let _ = input.parse::<Option<Token![;]>>()?;

        Ok(Self {
            file_path,
            wildcard,
            items,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_wildcard() {
        let input: BindJsInput = syn::parse_str(r#""src/dom.ts"::*"#).unwrap();
        assert_eq!(input.file_path.value(), "src/dom.ts");
        assert!(input.wildcard);
        assert!(input.items.is_empty());
    }

    #[test]
    fn test_parse_block() {
        let input: BindJsInput = syn::parse_str(
            r#"
            "src/dom.ts":: {
                focusElement,
                #[command] playSound as play_sound,
                #[query] getRect,
                #[watcher] watchScroll as on_scroll,
                #[watcher(raf)] watchResize as on_resize,
                *
            }
            "#,
        )
        .unwrap();

        assert_eq!(input.file_path.value(), "src/dom.ts");
        assert!(input.wildcard);
        assert_eq!(input.items.len(), 5);

        assert_eq!(input.items[0].original_name, "focusElement");
        assert_eq!(input.items[0].mode, None);
        assert_eq!(input.items[0].rename_as, None);

        assert_eq!(input.items[1].original_name, "playSound");
        assert_eq!(input.items[1].mode, Some(InteropMode::Command));
        assert_eq!(input.items[1].rename_as.as_ref().unwrap().to_string(), "play_sound");

        assert_eq!(input.items[2].original_name, "getRect");
        assert_eq!(input.items[2].mode, Some(InteropMode::Query));

        assert_eq!(input.items[3].original_name, "watchScroll");
        assert_eq!(input.items[3].mode, Some(InteropMode::Watcher { is_raf: false }));
        assert_eq!(input.items[3].rename_as.as_ref().unwrap().to_string(), "on_scroll");

        assert_eq!(input.items[4].original_name, "watchResize");
        assert_eq!(input.items[4].mode, Some(InteropMode::Watcher { is_raf: true }));
        assert_eq!(input.items[4].rename_as.as_ref().unwrap().to_string(), "on_resize");
    }
}
