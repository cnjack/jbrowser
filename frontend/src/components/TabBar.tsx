import type { BrowserTab } from '../api/types';

export type TabCommandPayload =
  | { command: 'activate'; tabId: string }
  | { command: 'close'; tabId: string }
  | { command: 'open'; url: string };

interface Props {
  tabs: BrowserTab[];
  onCommand: (payload: TabCommandPayload) => void;
}

export function TabBar({ tabs, onCommand }: Props) {
  return (
    <div className="tabs">
      {tabs.map((tab) => (
        <button
          className={tab.active ? 'tab active' : 'tab'}
          key={tab.id}
          onClick={() => onCommand({ command: 'activate', tabId: tab.id })}
          type="button"
          title={tab.url || tab.title}
        >
          {tab.favicon_url && (
            <img src={tab.favicon_url} alt="" width={14} height={14} className="tab-favicon" />
          )}
          <span className="tab-title">{tab.title || tab.url || 'New Tab'}</span>
          <span
            className="tab-close"
            onClick={(e) => { e.stopPropagation(); onCommand({ command: 'close', tabId: tab.id }); }}
          >
            ×
          </span>
        </button>
      ))}
      <button
        className="tab-new"
        type="button"
        onClick={() => onCommand({ command: 'open', url: 'about:blank' })}
        title="New tab"
      >
        +
      </button>
    </div>
  );
}
