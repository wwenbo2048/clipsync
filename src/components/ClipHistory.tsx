import { ClipboardEntry } from "../App";

interface ClipHistoryProps {
  entries: ClipboardEntry[];
  onCopy: (text: string) => void;
  onClear: () => void;
  onDownload: (transferId: string) => void;
  onOpenFolder: (filePath: string) => void;
}

export function ClipHistory({ entries, onCopy, onClear, onDownload, onOpenFolder }: ClipHistoryProps) {
  if (entries.length === 0) {
    return (
      <div className="empty-state">
        <div className="empty-icon">📋</div>
        <p>暂无剪贴板记录</p>
        <p className="empty-hint">复制文本或文件后将自动同步到所有已连接设备</p>
      </div>
    );
  }

  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp * 1000);
    return date.toLocaleTimeString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  };

  const truncateText = (text: string, maxLen: number) => {
    if (text.length <= maxLen) return text;
    return text.slice(0, maxLen) + "...";
  };

  const formatFileSize = (bytes: number | null) => {
    if (bytes == null) return "";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  };

  return (
    <div className="clip-history">
      <div className="history-header">
        <span>共 {entries.length} 条记录</span>
        <button className="clear-btn" onClick={onClear}>
          清空
        </button>
      </div>
      <div className="history-list">
        {entries.map((entry) => {
          const isFile = entry.content_type === "file";

          if (isFile) {
            // 判断是否是多文件或文件夹
            const isMultiOrFolder = entry.text.includes("个文件") || (entry.text.indexOf("/") >= 0);
            const fileIcon = isMultiOrFolder ? "📁" : "📄";

            return (
              <div
                key={entry.id}
                className="history-item history-item-file"
              >
                <div className="file-header">
                  <span className="file-icon">{fileIcon}</span>
                  <div className="file-info">
                    <div className="file-name" title={entry.text}>
                      {truncateText(entry.text, 40)}
                    </div>
                    <div className="file-size">{formatFileSize(entry.file_size)}</div>
                  </div>
                  <div className="file-action">
                    {entry.download_status === "pending" && entry.transfer_id && (
                      <button
                        className="download-btn"
                        onClick={(e) => {
                          e.stopPropagation();
                          onDownload(entry.transfer_id!);
                        }}
                      >
                        下载
                      </button>
                    )}
                    {entry.download_status === "downloading" && (
                      <span className="download-status downloading">下载中...</span>
                    )}
                    {entry.download_status === "done" && (
                      <button
                        className="download-btn open-folder-btn"
                        onClick={(e) => {
                          e.stopPropagation();
                          if (entry.file_path) onOpenFolder(entry.file_path);
                        }}
                      >
                        📂 打开文件夹
                      </button>
                    )}
                    {entry.download_status == null && (
                      <span className="download-status done">本机</span>
                    )}
                  </div>
                </div>
                <div className="history-meta">
                  <span className="history-source">{entry.source}</span>
                  <span className="history-time">{formatTime(entry.timestamp)}</span>
                </div>
              </div>
            );
          }

          // 文本条目
          return (
            <div
              key={entry.id}
              className="history-item"
              onClick={() => onCopy(entry.text)}
              title={entry.text}
            >
              <div className="history-text-row">
                <div className="history-text">{truncateText(entry.text, 80)}</div>
                <button
                  className="copy-btn"
                  onClick={(e) => {
                    e.stopPropagation();
                    onCopy(entry.text);
                  }}
                  title="复制"
                >
                  复制
                </button>
              </div>
              <div className="history-meta">
                <span className="history-source">{entry.source}</span>
                <span className="history-time">{formatTime(entry.timestamp)}</span>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
