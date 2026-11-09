mod adapter;

use ql_rpc::{
    download, duplex, notification, progress, request, subscription, upload, Route, RpcOptions,
    RpcRouteKey,
};

use crate::{QlStream, QlStreamError, RuntimeHandle, StreamOptions, StreamReader, StreamWriter};

type RpcResult<T> = Result<T, ql_rpc::RpcError<QlStreamError>>;

#[derive(Clone)]
pub struct RpcHandle {
    inner: RuntimeHandle,
    rpc_options: RpcOptions,
}

impl RpcHandle {
    pub fn options(mut self, options: RpcOptions) -> Self {
        self.rpc_options = options;
        self
    }

    pub async fn notification<M>(&self, event: &M::Payload, options: StreamOptions) -> RpcResult<()>
    where
        M: notification::Notification,
    {
        let stream = self.open_rpc_stream::<M>(options).await?;
        notification::send::<M, _>(stream, event)
            .await
            .map_err(ql_rpc::RpcError::Transport)
    }

    pub async fn request<M>(
        &self,
        request: &M::Request,
        options: StreamOptions,
    ) -> RpcResult<M::Response>
    where
        M: request::Request,
    {
        let stream = self.open_rpc_stream::<M>(options).await?;
        request::call::<M, _>(stream, request, self.rpc_options).await
    }

    pub async fn subscribe<M>(
        &self,
        request: &M::Request,
        options: StreamOptions,
    ) -> RpcResult<subscription::SubscriptionCall<M, StreamReader>>
    where
        M: subscription::Subscription,
    {
        let stream = self.open_rpc_stream::<M>(options).await?;
        subscription::start::<M, _>(stream, request, self.rpc_options).await
    }

    pub async fn download<M>(
        &self,
        request: &M::Request,
        options: StreamOptions,
    ) -> RpcResult<download::DownloadCall<M, StreamReader>>
    where
        M: download::Download,
    {
        let stream = self.open_rpc_stream::<M>(options).await?;
        download::start::<M, _>(stream, request, self.rpc_options).await
    }

    pub async fn progress<M>(
        &self,
        request: &M::Request,
        options: StreamOptions,
    ) -> RpcResult<progress::ProgressCall<M, StreamReader>>
    where
        M: progress::Progress,
    {
        let stream = self.open_rpc_stream::<M>(options).await?;
        progress::start::<M, _>(stream, request, self.rpc_options).await
    }

    pub async fn upload<M>(
        &self,
        request: &M::Request,
        options: StreamOptions,
    ) -> RpcResult<upload::UploadCall<M, StreamWriter, StreamReader>>
    where
        M: upload::Upload,
    {
        let stream = self.open_rpc_stream::<M>(options).await?;
        upload::start::<M, _>(stream, request, self.rpc_options)
            .await
            .map_err(ql_rpc::RpcError::Transport)
    }

    pub async fn duplex<M>(
        &self,
        options: StreamOptions,
    ) -> RpcResult<duplex::DuplexCall<M, StreamWriter, StreamReader>>
    where
        M: duplex::Duplex,
    {
        let stream = self.open_rpc_stream::<M>(options).await?;
        Ok(duplex::start::<M, _>(stream, self.rpc_options))
    }
}

impl RpcHandle {
    pub(super) fn new(inner: RuntimeHandle) -> Self {
        Self {
            inner,
            rpc_options: RpcOptions::default(),
        }
    }

    async fn open_rpc_stream<R>(&self, options: StreamOptions) -> RpcResult<QlStream>
    where
        R: Route,
        R::Key: RpcRouteKey,
    {
        let key = R::key();
        let mut header = Vec::with_capacity(key.encoded_len());
        key.encode(&mut header);
        self.inner
            .open_stream(header.into_boxed_slice(), options)
            .await
            .map_err(QlStreamError::from)
            .map_err(ql_rpc::RpcError::Transport)
    }
}
