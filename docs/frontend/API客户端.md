# Web API 客户端

接口字段与 HTTP 操作以 [OpenAPI](../../contracts/openapi.json) 为准；执行 `task api:generate`
从 Rust 声明重新生成契约和 `src/lib/generated/api.ts`，不要手改生成文件。
前端使用 openapi-fetch 按方法、路径推导请求和响应类型，`src/lib/api.ts` 统一同源 cookie、
错误文案及 multipart 序列化。生成类型仅提供编译期约束，业务校验仍由后端执行。
浏览器类型生成时省略自动发送的 Origin 请求头参数，将 binary 文件字段映射为 Blob/File；
正式契约仍保留服务端的完整要求。文件上传使用 FormData，由浏览器生成 Content-Type 和 boundary。
结构化 JSON 错误保留字段提示；原生文本及代理错误使用状态码对应的中文提示，网络异常交给 UI。
生成与检查入口见[服务端文档](../server/README.md#api-契约)。
