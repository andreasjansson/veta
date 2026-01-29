interface Env {
  OPENAI_API_KEY: string;
  Chat: DurableObjectNamespace<import("./src/agent").Chat>;
  VETA: Fetcher;
  VETA_DB: D1Database;
}

declare namespace NodeJS {
  interface ProcessEnv {
    OPENAI_API_KEY: string;
  }
}
