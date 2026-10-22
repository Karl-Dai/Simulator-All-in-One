import type { SuiteAPI } from "./types";

declare global {
  interface Window {
    go?: { main?: { App?: SuiteAPI } };
  }
}

export function backend(): SuiteAPI {
  const api = window.go?.main?.App;
  if (!api)
    throw new Error(
      "请从完整桌面套件打开 SimLab。网页预览无法启动本机模拟器。",
    );
  return api;
}
