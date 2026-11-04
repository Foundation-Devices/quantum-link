use std::future::Future;

use ql_codec::Encode;

use crate::{
    duplex::DuplexSender, rpc::read_eof_request, subscription::Subscription, Context, DecodeOwned,
    RouterConfig, RpcError, RpcRead, RpcStream, RpcWrite,
};

pub type SubscriptionResponder<T, W> = DuplexSender<T, W>;

#[trait_variant::make(SubscriptionHandler: Send)]
pub trait SubscriptionHandlerLocal<M, St>
where
    M: Subscription,
    St: RpcStream,
{
    async fn handle(
        self,
        context: Context,
        message: M::Request,
        responder: DuplexSender<M::Event, St::Writer>,
    );

    fn handle_error(&self, error: &RpcError<St::Error>) {
        let _ = error;
    }
}

pub(crate) fn handle_subscription<S, Req, Event, St, H, HF, E>(
    state: S,
    context: Context,
    config: RouterConfig,
    stream: St,
    handle: H,
    handle_error: E,
) -> impl Future<Output = ()>
where
    Req: DecodeOwned + 'static,
    Event: Encode + 'static,
    St: RpcStream + 'static,
    H: FnOnce(S, Context, Req, DuplexSender<Event, St::Writer>) -> HF,
    HF: Future<Output = ()>,
    E: FnOnce(&S, &RpcError<St::Error>),
{
    let (mut reader, mut writer) = stream.split();

    async move {
        let request = match read_eof_request::<Req, _>(&mut reader, config).await {
            Ok(request) => request,
            Err(error) => {
                let code = error.reset_code();
                handle_error(&state, &error);
                if let Some(code) = code {
                    reader.reset(code);
                    writer.reset(code);
                }
                return;
            }
        };

        handle(state, context, request, DuplexSender::new(writer)).await;
    }
}
