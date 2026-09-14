import {
  ArrowDown,
  ArrowUpRight,
  Check,
  Download,
  GitBranch,
  Laptop,
  Layers3,
  Monitor,
  Terminal,
  Workflow,
} from "lucide-react";
import Gradient from "./components/gradient";
import HeroGraph from "./components/hero-graph";
import { LatestReleaseLink, ReleaseDownload } from "./components/releases";
import WorkspaceDemo from "./components/workspace-demo";
import { PrivacySettingsButton } from "./components/analytics";

const repo = "https://github.com/Jacqkues/schematlas";

function Github({ size = 19 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="currentColor"
      aria-hidden="true"
    >
      <path d="M12 .75a11.25 11.25 0 0 0-3.558 21.923c.562.105.77-.244.77-.542 0-.267-.01-.975-.015-1.914-3.13.68-3.79-1.51-3.79-1.51-.512-1.3-1.25-1.646-1.25-1.646-1.022-.699.078-.685.078-.685 1.13.08 1.725 1.16 1.725 1.16 1.006 1.724 2.64 1.226 3.283.938.102-.73.393-1.226.714-1.508-2.498-.284-5.125-1.249-5.125-5.561 0-1.229.44-2.232 1.16-3.019-.116-.285-.503-1.43.11-2.98 0 0 .945-.303 3.093 1.153A10.78 10.78 0 0 1 12 6.182c.955.005 1.916.13 2.812.378 2.148-1.456 3.09-1.153 3.09-1.153.616 1.55.229 2.695.112 2.98.722.787 1.16 1.79 1.16 3.019 0 4.323-2.631 5.274-5.138 5.553.404.35.764 1.043.764 2.1 0 1.518-.014 2.743-.014 3.115 0 .301.204.653.775.542A11.251 11.251 0 0 0 12 .75Z" />
    </svg>
  );
}
function Brand() {
  return (
    <a href="#top" className="brand" aria-label="Schematlas home">
      <img src="/favicon.svg" alt="" width={32} height={32} />
      <span>
        schem<strong>atlas</strong>
      </span>
    </a>
  );
}

export default function Home() {
  return (
    <div id="top">
      <a className="skip-link" href="#main">
        Skip to content
      </a>
      <main id="main">
        <section className="hero hero-with-graph" aria-labelledby="hero-title">
          <Gradient />
          <div className="hero-composition">
          <div className="hero-content">
            <div className="hero-brand">
              <img src="/favicon.svg" alt="" width={36} height={36} />
              <span>
                schem<strong>atlas</strong>
              </span>
            </div>
            <h1 id="hero-title">
              <span className="headline-line">See how it</span>
              <span className="headline-line headline-accent">
                all connects.
              </span>
            </h1>
            <p className="hero-description">
              Your databases, APIs, and coding agents.
              <br />
              <span>One visual workspace, right on your desktop.</span>
            </p>
            <div className="hero-actions">
              <a href="#download" className="button primary">
                <Download size={17} />
                Download Schematlas
                <ArrowUpRight size={16} />
              </a>
              <a href={repo} className="hero-code-link">
                <Github size={18} />
                Explore the code
                <ArrowUpRight size={14} />
              </a>
            </div>
            <p className="hero-note">
              Free & open source <span>·</span> macOS, Windows & Linux
            </p>
          </div>
          <HeroGraph />
          </div>
          <div className="hero-footer">
            <LatestReleaseLink hero />
            <a href="#workspace" className="scroll-cue">
              A CLEARER PICTURE BELOW
              <ArrowDown size={15} />
            </a>
          </div>
        </section>
        <section
          id="workspace"
          className="product container"
          aria-label="Schematlas app preview"
        >
          <div className="product-topline">
            <span>
              <span className="tiny-square" /> YOUR SYSTEM, MAPPED
            </span>
            <span>Less guesswork. More context.</span>
          </div>
          <h2 className="product-title">
            Complex systems.
            <br />
            <span>Clear perspective.</span>
          </h2>
          <WorkspaceDemo />
        </section>
        <section
          className="connections container"
          aria-label="Supported databases"
        >
          <p>Bring the tools you already work with.</p>
          <div>
            <span>
              <img
                src="/logos/postgresql.svg"
                className="database-logo database-logo--postgresql"
                alt=""
                width={34}
                height={34}
                loading="lazy"
              />
              PostgreSQL
            </span>
            <span>
              <img
                src="/logos/mysql.svg"
                className="database-logo database-logo--mysql"
                alt=""
                width={34}
                height={34}
                loading="lazy"
              />
              MySQL
            </span>
            <span>
              <img
                src="/logos/mariadb.svg"
                className="database-logo database-logo--mariadb"
                alt=""
                width={34}
                height={34}
                loading="lazy"
              />
              MariaDB
            </span>
            <span>
              <img
                src="/logos/sqlite.svg"
                className="database-logo database-logo--sqlite"
                alt=""
                width={34}
                height={34}
                loading="lazy"
              />
              SQLite
            </span>
            <span>
              <img
                src="/logos/microsoftsqlserver.svg"
                className="database-logo database-logo--microsoftsqlserver"
                alt=""
                width={34}
                height={34}
                loading="lazy"
              />
              SQL Server
            </span>
            <span>
              <Workflow />
              OpenAPI
            </span>
          </div>
        </section>
        <section id="explore" className="explore container">
          <div className="section-heading">
            <span className="eyebrow">FROM STRUCTURE TO UNDERSTANDING</span>
            <h2>
              A little less digging.
              <br />
              <span>A lot more clarity.</span>
            </h2>
            <p>
              Get the context you need before you write the next query, change
              an endpoint, or ask your agent to help.
            </p>
          </div>
          <div className="features">
            <article>
              <div className="feature-icon">
                <GitBranch size={22} />
              </div>
              <span className="feature-number">01 / DATABASES</span>
              <h3>Follow every relationship.</h3>
              <p>
                Explore tables across multiple schemas. Follow foreign keys, see
                cardinality, and bring related tables into focus with a click.
              </p>
              <div className="feature-foot">
                <Check size={14} /> Multi-schema connections
              </div>
            </article>
            <article>
              <div className="feature-icon">
                <Layers3 size={22} />
              </div>
              <span className="feature-number">02 / APIS</span>
              <h3>Give your API a shape.</h3>
              <p>
                Import an OpenAPI JSON file and see its endpoints, models, and
                references as a connected graph, alongside your databases.
              </p>
              <div className="feature-foot">
                <Check size={14} /> OpenAPI 3 & Swagger 2
              </div>
            </article>
            <article>
              <div className="feature-icon">
                <Terminal size={22} />
              </div>
              <span className="feature-number">03 / CODING AGENTS</span>
              <h3>Bring your agent into the picture.</h3>
              <p>
                Connect an ACP-compatible coding agent. Let it inspect schemas
                and organize the canvas. Review SQL and HTTP requests before
                they run.
              </p>
              <div className="feature-foot">
                <Check size={14} /> Agent sessions, in context
              </div>
            </article>
          </div>
        </section>
        <section
          id="download"
          className="downloads container"
          aria-labelledby="download-title"
        >
          <div className="download-heading">
            <div>
              <span className="eyebrow">MAKE YOURSELF AT HOME</span>
              <h2 id="download-title">
                Your next clear idea
                <br />
                starts here.
              </h2>
            </div>
            <div>
              <p>
                Pick your platform. Open your first project.
                <br />
                Start connecting the dots.
              </p>
              <LatestReleaseLink />
            </div>
          </div>
          <div className="platforms">
            <article>
              <Laptop size={28} />
              <h3>macOS</h3>
              <p>For your Mac.</p>
              <ReleaseDownload kind="macos-arm64.dmg">
                Apple silicon{" "}
                <span>
                  DMG <Download size={15} />
                </span>
              </ReleaseDownload>
              <ReleaseDownload kind="macos-x64.dmg">
                Intel{" "}
                <span>
                  DMG <Download size={15} />
                </span>
              </ReleaseDownload>
            </article>
            <article>
              <Monitor size={28} />
              <h3>Windows</h3>
              <p>For your PC. 64-bit.</p>
              <ReleaseDownload kind="windows-x64.exe">
                Installer{" "}
                <span>
                  EXE <Download size={15} />
                </span>
              </ReleaseDownload>
              <ReleaseDownload kind="windows-x64.msi">
                Windows Installer{" "}
                <span>
                  MSI <Download size={15} />
                </span>
              </ReleaseDownload>
            </article>
            <article>
              <Terminal size={28} />
              <h3>Linux</h3>
              <p>For your favorite distro. x64.</p>
              <ReleaseDownload kind="linux-x64.AppImage">
                Portable app{" "}
                <span>
                  AppImage <Download size={15} />
                </span>
              </ReleaseDownload>
              <ReleaseDownload kind="linux-x64.deb">
                Debian / Ubuntu{" "}
                <span>
                  DEB <Download size={15} />
                </span>
              </ReleaseDownload>
            </article>
          </div>
          <p className="install-note">
            On macOS, the unsigned app may need to be allowed in Privacy &
            Security.{" "}
            <a href={`${repo}/blob/main/docs/macos-releases.md`}>
              Installation help <ArrowUpRight size={13} />
            </a>
          </p>
        </section>
        <section className="open-source container">
          <div className="open-source-symbol">
            <Github size={36} />
          </div>
          <div>
            <span className="eyebrow">BUILT IN THE OPEN</span>
            <h2>Good tools belong to everyone.</h2>
            <p>
              Read the code. Open an issue. Make it your own.
              <br />
              Schematlas is free software, licensed under Apache 2.0.
            </p>
          </div>
          <a href={repo} className="button secondary">
            View on GitHub
            <ArrowUpRight size={17} />
          </a>
        </section>
      </main>
      <footer className="container">
        <Brand />
        <p>Your data, mapped.</p>
        <div>
          <a href={`${repo}/blob/main/LICENSE`}>Apache 2.0</a>
          <a href={`${repo}/issues`}>Issues</a>
          <PrivacySettingsButton />
          <a href="https://shadergradient.co/">
            Gradient by Shader Gradient
            <ArrowUpRight size={12} />
          </a>
        </div>
      </footer>
    </div>
  );
}
