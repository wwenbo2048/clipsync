import { ClipboardEntry } from "../App";
import { useI18n } from "../i18n/I18nContext";

interface ClipHistoryProps {
  entries: ClipboardEntry[];
  onCopy: (text: string) => void;
  onClear: () => void;
  onDownload: (transferId: string) => void;
  onOpenFolder: (filePath: string) => void;
}

export function ClipHistory({ entries, onCopy, onClear, onDownload, onOpenFolder }: ClipHistoryProps) {
  const { t, locale } = useI18n();

  if (entries.length === 0) {
    return (
      <div className="empty-state">
        <div className="empty-icon">📋</div>
        <p>{t.history_empty}</p>
        <p className="empty-hint">{t.history_empty_hint}</p>
      </div>
    );
  }

  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp * 1000);
    return date.toLocaleTimeString(locale, {
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
        <span>{t.history_count(entries.length)}</span>
        <button className="clear-btn" onClick={onClear}>
          {t.history_clear}
        </button>
      </div>
      <div className="history-list">
        {entries.map((entry) => {
          const isFile = entry.content_type === "file";
          const isImage = entry.content_type === "image";

          if (isImage) {
            return (
              <div
                key={entry.id}
                className="history-item history-item-file"
              >
                <div className="file-header">
                  <span className="file-icon">🖼️</span>
                  <div className="file-info">
                    <div className="file-name">
                      {t.history_image} ({formatFileSize(entry.file_size)})
                    </div>
                  </div>
                  <div className="file-action">
                    <span className="download-status done">
                      {entry.source === "本机" ? t.history_local : "✓"}
                    </span>
                  </div>
                </div>
                <div className="history-meta">
                  <span className="history-source">{entry.source}</span>
                  <span className="history-time">{formatTime(entry.timestamp)}</span>
                </div>
              </div>
            );
          }

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
                        {t.history_download}
                      </button>
                    )}
                    {entry.download_status === "downloading" && (
                      <span className="download-status downloading">{t.history_downloading}</span>
                    )}
                    {entry.download_status === "done" && (
                      <button
                        className="download-btn open-folder-btn"
                        onClick={(e) => {
                          e.stopPropagation();
                          if (entry.file_path) onOpenFolder(entry.file_path);
                        }}
                      >
                        {t.history_open_folder}
                      </button>
                    )}
                    {entry.download_status == null && (
                      <span className="download-status done">{t.history_local}</span>
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
                  title={t.history_copy}
                >
                  {t.history_copy}
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
