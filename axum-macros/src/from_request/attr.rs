use crate::ParseAttrs;
use syn::Path;

parse_arg!(Via: via(Path));
parse_arg!(Rejection: rejection(Path));

#[derive(Debug)]
pub(super) enum ContainerAttrs {
    Via { path: Path, rejection: Option<Path> },
    Rejection { path: Path },
    Default,
}

#[derive(Debug, Default)]
pub(super) struct RawContainerAttrs {
    via: Option<Path>,
    rejection: Option<Path>,
}

impl RawContainerAttrs {
    pub(super) fn validate(self) -> syn::Result<ContainerAttrs> {
        let Self { via, rejection } = self;
        match (via, rejection) {
            (None, None) => Ok(ContainerAttrs::Default),
            (None, Some(rejection)) => Ok(ContainerAttrs::Rejection { path: rejection }),
            (Some(via), None) => Ok(ContainerAttrs::Via {
                path: via,
                rejection: None,
            }),
            (Some(via), Some(rejection)) => Ok(ContainerAttrs::Via {
                path: via,
                rejection: Some(rejection),
            }),
        }
    }
}

impl ParseAttrs for RawContainerAttrs {
    const IDENT: &'static str = "from_request";

    type Arg = ContainerAttrArg;

    fn merge(mut self, attr: Self::Arg) -> syn::Result<Self> {
        match attr {
            ContainerAttrArg::Via(Via { kw, via }) => {
                if self.via.is_some() {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "`via` specified more than once",
                    ));
                }
                self.via = Some(via);
            }
            ContainerAttrArg::Rejection(Rejection { kw, rejection }) => {
                if self.rejection.is_some() {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "`rejection` specified more than once",
                    ));
                }
                self.rejection = Some(rejection);
            }
        }

        Ok(self)
    }
}

parse_args_enum! {
    pub(crate) enum ContainerAttrArg {
        Via,
        Rejection,
    }
}

#[derive(Debug)]
pub(super) enum FieldAttrs {
    Via { path: Path, kw: via::via },
    Default,
}

impl Default for FieldAttrs {
    fn default() -> Self {
        Self::Default
    }
}

impl ParseAttrs for FieldAttrs {
    const IDENT: &'static str = "from_request";

    type Arg = FieldAttrArg;

    fn merge(self, attr: Self::Arg) -> syn::Result<Self> {
        match (self, attr) {
            (FieldAttrs::Default, FieldAttrArg::Via(Via { via, kw })) => {
                Ok(Self::Via { path: via, kw })
            }
            (FieldAttrs::Via { .. }, FieldAttrArg::Via(Via { kw, .. })) => Err(
                syn::Error::new_spanned(kw, "`via` specified more than once"),
            ),
        }
    }
}

parse_args_enum! {
    pub(crate) enum FieldAttrArg {
        Via,
    }
}
