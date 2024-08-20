use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_derive(AppStopSignal)]
pub fn stop_signal_derive(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).unwrap();
    let name = &ast.ident;

    quote! {
        impl actix::Handler<crate::app::signal::Stop> for #name {
            type Result = crate::app::signal::StopResult;

            fn handle(&mut self, _msg: app::signal::Stop, ctx: &mut Self::Context) -> Self::Result {
                use actix::ActorContext;

                ctx.stop();
                core::result::Result::Ok(())
            }
        }
    }
    .into()
}

#[proc_macro_derive(AppTerminateSignal)]
pub fn terminate_signal_derive(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).unwrap();
    let name = &ast.ident;

    quote! {
        impl actix::Handler<crate::app::signal::Terminate> for #name {
            type Result = ();

            fn handle(
                &mut self,
                _msg: app::signal::Terminate,
                ctx: &mut Self::Context,
            ) -> Self::Result {
                use actix::ActorContext;
                ctx.terminate()
            }
        }
    }
    .into()
}
