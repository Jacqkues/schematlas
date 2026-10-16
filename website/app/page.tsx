import { ArrowDown, ArrowUpRight, Terminal } from 'lucide-react';
import Aurora from './components/aurora';
import AppTour from './components/app-tour';
import PlatformDownloads, { PlatformDownloadButton } from './components/platform-downloads';
import { LatestReleaseLink } from './components/releases';
import { PrivacySettingsButton } from './components/analytics';

const repo = 'https://github.com/Jacqkues/schematlas';

function Github({ size = 19 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <path d="M12 .75a11.25 11.25 0 0 0-3.558 21.923c.562.105.77-.244.77-.542 0-.267-.01-.975-.015-1.914-3.13.68-3.79-1.51-3.79-1.51-.512-1.3-1.25-1.646-1.25-1.646-1.022-.699.078-.685.078-.685 1.13.08 1.725 1.16 1.725 1.16 1.006 1.724 2.64 1.226 3.283.938.102-.73.393-1.226.714-1.508-2.498-.284-5.125-1.249-5.125-5.561 0-1.229.44-2.232 1.16-3.019-.116-.285-.503-1.43.11-2.98 0 0 .945-.303 3.093 1.153A10.78 10.78 0 0 1 12 6.182c.955.005 1.916.13 2.812.378 2.148-1.456 3.09-1.153 3.09-1.153.616 1.55.229 2.695.112 2.98.722.787 1.16 1.79 1.16 3.019 0 4.323-2.631 5.274-5.138 5.553.404.35.764 1.043.764 2.1 0 1.518-.014 2.743-.014 3.115 0 .301.204.653.775.542A11.251 11.251 0 0 0 12 .75Z" />
    </svg>
  );
}
function Brand() {
  return (
    <a href="#top" className="m-brand" aria-label="Schematlas home">
      <img src="/favicon.svg" alt="" width={24} height={24} />
      <span>Schematlas</span>
    </a>
  );
}

const steps = [
  [
    'Open your repository',
    'The Mermaid, draw.io and DBML files you already have, and a wiki for what is missing.',
  ],
  [
    'Ask your agent',
    'Codex, Claude Code or any ACP agent maps a flow or writes a page. You review every change.',
  ],
  [
    'Follow it to the code',
    'Each node cites its source. When that code changes on main, the diagram tells you.',
  ],
];

export default function Home() {
  return (
    <div id="top" className="m-page">
      <a className="skip-link" href="#main">
        Skip to content
      </a>
      <header className="m-nav">
        <Brand />
        <nav aria-label="Main navigation">
          <a href="#workspace">Tour</a>
          <a href="#how-it-works">How it works</a>
          <a href="/docs">Docs</a>
          <a href={repo} className="m-nav-icon" aria-label="GitHub">
            <Github size={16} />
          </a>
          <a href="#download" className="m-nav-download">
            Download
          </a>
        </nav>
      </header>
      <main id="main">
        <section className="m-hero" aria-labelledby="hero-title">
          <p className="m-label">[ Free &amp; open source · macOS · Windows · Linux ]</p>
          <h1 id="hero-title">
            <span className="m-line">Your codebase.</span>
            <span className="m-line m-shine">A clearer picture.</span>
          </h1>
          <p className="m-lede">
            Diagrams and documentation for your repository. Written with your agent, kept in your
            code.
          </p>
          <div className="m-actions">
            <PlatformDownloadButton />
            <a href="#workspace" className="m-link">
              Take the tour <ArrowDown size={15} />
            </a>
          </div>
          <a href="#agent-install" className="m-agent-hero">
            Install with your agent <ArrowUpRight size={13} />
          </a>
        </section>

        <AppTour />

        <section id="how-it-works" className="m-section" aria-labelledby="steps-title">
          <div className="m-head">
            <p className="m-label">[ 01 — How it works ]</p>
            <h2 id="steps-title">
              Less reconstructing.
              <br />
              More understanding.
            </h2>
          </div>
          <ol className="m-steps">
            {steps.map(([title, text], i) => (
              <li key={title}>
                <span className="m-index">0{i + 1}</span>
                <h3>{title}</h3>
                <p>{text}</p>
              </li>
            ))}
          </ol>
          <div className="m-strip" aria-label="Agents and file formats">
            <span>
              <img src="/logos/openai.svg" width="17" height="17" alt="" /> Codex
            </span>
            <span>
              <img src="/logos/claude.svg" width="17" height="17" alt="" /> Claude Code
            </span>
            <span>
              <Terminal size={15} /> ACP agents
            </span>
            <i />
            <span className="m-mono">Mermaid</span>
            <span className="m-mono">draw.io</span>
            <span className="m-mono">DBML</span>
            <span className="m-mono">Markdown</span>
          </div>
        </section>

        <section className="m-section m-split" aria-labelledby="files-title">
          <div className="m-head">
            <p className="m-label">[ 02 — Plain files ]</p>
            <h2 id="files-title">
              In your repo.
              <br />
              On your terms.
            </h2>
            <p>
              Readable without Schematlas, reviewed in your pull requests, versioned with your code.
            </p>
            <a className="m-link" href={`${repo}/blob/main/docs/toc.md`}>
              How the files fit together <ArrowUpRight size={14} />
            </a>
          </div>
          <pre className="m-tree" aria-label="Example documentation files">
            <span>your-repository/</span>
            {'\n'}└─ .schematlas/
            {'\n'} ├─ toc.yaml <em>the way in</em>
            {'\n'} ├─ wiki/overview.md <em>the explanation</em>
            {'\n'} └─ diagrams/
            {'\n'} ├─ architecture.mmd <em>the big picture</em>
            {'\n'} └─ checkout.mmd <em>the details</em>
          </pre>
        </section>

        <section id="storage" className="m-section" aria-labelledby="storage-title">
          <div className="m-head">
            <p className="m-label">[ 03 — Local first ]</p>
            <h2 id="storage-title">No account. No new platform.</h2>
          </div>
          <ul className="m-facts">
            <li>
              <h3>Files stay with your project.</h3>
              <p>Team sync is optional and goes through your own git remote.</p>
            </li>
            <li>
              <h3>You approve agent changes.</h3>
              <p>Edits, SQL and HTTP requests wait for you. Conversations stay with your agent.</p>
            </li>
            <li>
              <h3>Credentials stay in memory.</h3>
              <p>Connection strings and API headers are never saved or logged.</p>
            </li>
          </ul>
          <a className="m-link" href="/docs/sync-and-privacy">
            Privacy and storage details <ArrowUpRight size={14} />
          </a>
        </section>

        <section id="download" className="m-download" aria-labelledby="download-title">
          <Aurora palette="forest" seed={2} />
          <div className="m-download-inner">
            <p className="m-label">[ Download ]</p>
            <h2 id="download-title">Make yourself at home in your codebase.</h2>
            <PlatformDownloads releaseNotes={<LatestReleaseLink />} />
          </div>
        </section>
      </main>
      <footer className="m-footer">
        <Brand />
        <p>Free software, Apache 2.0.</p>
        <div>
          <a href={repo}>GitHub</a>
          <a href="/docs">Docs</a>
          <a href={`${repo}/issues`}>Issues</a>
          <PrivacySettingsButton />
        </div>
      </footer>
    </div>
  );
}
