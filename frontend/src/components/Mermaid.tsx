import mermaid from 'mermaid';
import { useEffect, useId, useRef, useState } from 'react';

mermaid.initialize({
  startOnLoad: false,
  theme: 'base',
  themeVariables: {
    // Warm paper palette
    background:           '#f7f1de',
    mainBkg:              '#ece4cf',
    nodeBorder:           'rgba(21,20,15,0.18)',
    clusterBkg:           '#f0e8d4',
    clusterBorder:        'rgba(21,20,15,0.14)',
    titleColor:           '#15140f',
    edgeLabelBackground:  '#f7f1de',
    lineColor:            '#8b8676',
    // Node text
    primaryColor:         '#ece4cf',
    primaryTextColor:     '#15140f',
    primaryBorderColor:   'rgba(21,20,15,0.22)',
    secondaryColor:       '#f0e8d4',
    secondaryTextColor:   '#15140f',
    secondaryBorderColor: 'rgba(21,20,15,0.18)',
    tertiaryColor:        '#f7f1de',
    tertiaryTextColor:    '#15140f',
    tertiaryBorderColor:  'rgba(21,20,15,0.18)',
    // Sequence diagram
    actorBkg:             '#ece4cf',
    actorBorder:          'rgba(21,20,15,0.22)',
    actorTextColor:       '#15140f',
    actorLineColor:       '#8b8676',
    signalColor:          '#5a5448',
    signalTextColor:      '#15140f',
    labelBoxBkgColor:     '#f7f1de',
    labelBoxBorderColor:  'rgba(21,20,15,0.14)',
    labelTextColor:       '#15140f',
    loopTextColor:        '#5a5448',
    noteBkgColor:         '#f0e8d4',
    noteBorderColor:      'rgba(21,20,15,0.18)',
    noteTextColor:        '#2a2620',
    activationBkgColor:   '#ddd2b6',
    activationBorderColor:'rgba(21,20,15,0.22)',
    fontFamily:           'Inter, -apple-system, system-ui, sans-serif',
    fontSize:             '13px',
  },
  flowchart: { htmlLabels: true, curve: 'basis' },
  sequence:  { useMaxWidth: true },
});

interface Props {
  chart: string;
}

export function Mermaid({ chart }: Props) {
  const id = useId().replace(/:/g, '');
  const ref = useRef<HTMLDivElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (!ref.current) return;
    const el = ref.current;
    mermaid.render(`m${id}`, chart.trim()).then(({ svg }) => {
      el.innerHTML = svg;
      const svgEl = el.querySelector('svg');
      if (svgEl) {
        svgEl.style.maxWidth = '100%';
        svgEl.style.height = 'auto';
      }
    });
  }, [chart, id]);

  // Render enlarged SVG into modal when opened
  useEffect(() => {
    if (!open || !modalRef.current) return;
    const el = modalRef.current;
    mermaid.render(`m${id}modal`, chart.trim()).then(({ svg }) => {
      el.innerHTML = svg;
      const svgEl = el.querySelector('svg');
      if (svgEl) {
        svgEl.style.maxWidth = '100%';
        svgEl.style.height = 'auto';
      }
    });
  }, [open, chart, id]);

  // Close on Escape
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => { if (e.key === 'Escape') setOpen(false); };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [open]);

  return (
    <>
      <div className="mermaid-wrap-outer">
        <div ref={ref} className="mermaid-wrap" />
        <button
          className="mermaid-zoom-btn"
          onClick={() => setOpen(true)}
          title="放大查看"
          aria-label="放大查看"
        >
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
            <path d="M1 6V1h5M1 1l5 5M10 1h5v5M15 1l-5 5M6 15H1v-5M1 15l5-5M15 10v5h-5M15 15l-5-5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
          </svg>
        </button>
      </div>

      {open && (
        <div className="mermaid-modal-overlay" onClick={() => setOpen(false)}>
          <div className="mermaid-modal" onClick={e => e.stopPropagation()}>
            <button
              className="mermaid-modal-close"
              onClick={() => setOpen(false)}
              aria-label="关闭"
            >
              <svg width="18" height="18" viewBox="0 0 18 18" fill="none">
                <path d="M2 2l14 14M16 2L2 16" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
              </svg>
            </button>
            <div ref={modalRef} className="mermaid-modal-body" />
          </div>
        </div>
      )}
    </>
  );
}
