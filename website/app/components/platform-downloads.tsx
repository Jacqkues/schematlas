'use client';

import { useEffect, useState, type ReactNode } from 'react';
import { Download } from 'lucide-react';
import { ReleaseDownload } from './releases';
import AgentInstall from './agent-install';

type Platform = 'macos' | 'windows' | 'linux';
const names = { macos: 'macOS', windows: 'Windows', linux: 'Linux' };

function usePlatform() {
  const [platform, setPlatform] = useState<Platform | null>(null);
  useEffect(() => {
    const agent = navigator.userAgent;
    // iPads can identify as Macs. Keep desktop downloads available without
    // recommending a Mac installer to a touch-only mobile device.
    if (
      /Android|iPhone|iPad|iPod/i.test(agent) ||
      (/Mac/i.test(navigator.platform) && navigator.maxTouchPoints > 1)
    )
      return;
    if (/Windows/i.test(agent)) setPlatform('windows');
    else if (/Mac/i.test(agent)) setPlatform('macos');
    else if (/Linux/i.test(agent)) setPlatform('linux');
  }, []);
  return platform;
}

export function PlatformDownloadButton() {
  const platform = usePlatform();
  const label = platform ? `Download for ${names[platform]}` : 'Download Schematlas';
  if (platform === 'windows' || platform === 'linux') {
    return (
      <ReleaseDownload
        kind={platform === 'windows' ? 'windows-x64.exe' : 'linux-x64.AppImage'}
        className="m-button"
      >
        {label}
      </ReleaseDownload>
    );
  }
  return (
    <a href="#download" className="m-button">
      {label}
    </a>
  );
}

export default function PlatformDownloads({ releaseNotes }: { releaseNotes: ReactNode }) {
  const platform = usePlatform();
  const [showAll, setShowAll] = useState(false);
  const visible = (value: Platform) => !platform || showAll || platform === value;
  return (
    <>
      <div className="m-platforms" data-platform={showAll ? 'all' : (platform ?? 'all')}>
        {visible('macos') && (
          <article>
            <h3>macOS</h3>
            <ReleaseDownload kind="macos-arm64.dmg">
              Apple silicon <Download size={14} />
            </ReleaseDownload>
            <ReleaseDownload kind="macos-x64.dmg">
              Intel <Download size={14} />
            </ReleaseDownload>
          </article>
        )}
        {visible('windows') && (
          <article>
            <h3>Windows</h3>
            <ReleaseDownload kind="windows-x64.exe">
              Download for Windows <Download size={14} />
            </ReleaseDownload>
            <details className="m-download-alternative">
              <summary>MSI installer</summary>
              <ReleaseDownload kind="windows-x64.msi">
                Download MSI <Download size={14} />
              </ReleaseDownload>
            </details>
          </article>
        )}
        {visible('linux') && (
          <article>
            <h3>Linux</h3>
            <ReleaseDownload kind="linux-x64.AppImage">
              Download AppImage <Download size={14} />
            </ReleaseDownload>
            <details className="m-download-alternative">
              <summary>Debian / Ubuntu</summary>
              <ReleaseDownload kind="linux-x64.deb">
                Download DEB <Download size={14} />
              </ReleaseDownload>
            </details>
          </article>
        )}
      </div>
      {platform && (
        <button
          type="button"
          className="m-other-platforms"
          aria-expanded={showAll}
          onClick={() => setShowAll((value) => !value)}
        >
          {showAll ? `Show ${names[platform]} only` : 'Other platforms'}
        </button>
      )}
      <AgentInstall />
      <p className="m-note">
        {releaseNotes}
        {visible('macos') && (
          <>
            <span>·</span> On macOS, the unsigned app may need to be allowed in Privacy &amp;
            Security.{' '}
            <a href="https://github.com/Jacqkues/schematlas/blob/main/docs/macos-releases.md">
              Installation help
            </a>
          </>
        )}
      </p>
    </>
  );
}
