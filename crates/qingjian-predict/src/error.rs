use thiserror::Error;

#[derive(Debug, Error)]
pub enum PredictError {
    #[error("no API key: set `api_key` in config or the `{0}` environment variable")]
    MissingApiKey(String),

    #[error("failed to start async runtime: {0}")]
    Runtime(#[from] std::io::Error),

    /// 后台线程还没回话就没了（panic 或者被提前关掉）。
    ///
    /// **必须与「还没回来」分开**：两种情况在通道上都表现为「取不到东西」，
    /// 混在一起的话调用方会一直等下去，界面上只看到「正在测试…」直到超时，
    /// 真正的原因一个字都露不出来（2026-09-22 用户撞上的就是这个）。
    #[error("the request worker exited before answering")]
    WorkerGone,

    #[error("API request failed: {0}")]
    Api(#[from] async_openai::error::OpenAIError),

    #[error("API request timed out after {0} ms")]
    Timeout(u64),

    #[error("API returned no usable content")]
    EmptyReply,
}
