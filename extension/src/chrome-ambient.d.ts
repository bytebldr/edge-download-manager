// Ambient declaration for WebExtension `chrome` API when @types/chrome is not installed.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
declare const chrome: any;
// eslint-disable-next-line @typescript-eslint/no-namespace
declare namespace chrome {
  // eslint-disable-next-line @typescript-eslint/no-namespace
  namespace downloads {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    type DownloadItem = any;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    type DownloadQuery = any;
  }
  // eslint-disable-next-line @typescript-eslint/no-namespace
  namespace runtime {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    type MessageSender = any;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    type Port = any;
  }
}
