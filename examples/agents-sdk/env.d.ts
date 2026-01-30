interface Env {
  OPENAI_API_KEY: string;
  Chat: DurableObjectNamespace<import("./src/agent").Chat>;
  VETA: Fetcher;
}

declare namespace NodeJS {
  interface ProcessEnv {
    OPENAI_API_KEY: string;
  }
}

declare module "veta" {
  const worker: ExportedHandler;
  export default worker;
}
