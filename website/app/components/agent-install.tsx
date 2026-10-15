'use client';

import { useEffect, useRef, useState } from 'react';
import { Check, Copy } from 'lucide-react';

const agents = [
  { name: 'Codex', logo: '/logos/openai.svg' },
  { name: 'Claude Code', logo: '/logos/claude.svg' },
];

export default function AgentInstall() {
  const [agent, setAgent] = useState('Codex');
  const [copied, setCopied] = useState(false);
  const [manual, setManual] = useState(false);
  const promptField = useRef<HTMLTextAreaElement>(null);
  const prompt = () =>
    `Install Schematlas from source on my computer. Read ${window.location.origin}/install.md and follow its instructions. Detect my operating system and CPU architecture, check the prerequisites, clone the official repository into a new directory, build the native desktop app, and help me install and open it. Follow the selected release's README and AGENTS.md. Preserve my existing projects and toolchain defaults. Tell me which release and commit you built and where the app is installed.`;
  const [text, setText] = useState('');
  useEffect(() => {
    if (manual) {
      promptField.current?.focus();
      promptField.current?.select();
    }
  }, [manual, text]);
  const copy = async () => {
    const value = prompt();
    setText(value);
    setCopied(false);
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setManual(false);
    } catch {
      setManual(true);
    }
  };
  return (
    <section id="agent-install" className="m-agent-install" aria-labelledby="agent-install-title">
      <h3 id="agent-install-title">Or let your agent build it.</h3>
      <p>Copy a prompt. Paste it into your agent. Let it handle the setup.</p>
      <div className="m-agent-options" role="group" aria-label="Choose your coding agent">
        {agents.map((item) => (
          <button
            key={item.name}
            type="button"
            aria-pressed={agent === item.name}
            onClick={() => {
              setAgent(item.name);
              setCopied(false);
            }}
          >
            <img src={item.logo} width={17} height={17} alt="" />
            {item.name}
          </button>
        ))}
      </div>
      <button type="button" className="m-agent-copy" onClick={copy}>
        {copied ? <Check size={15} /> : <Copy size={15} />}
        {copied ? 'Prompt copied' : `Ask ${agent} to install`}
      </button>
      <p role="status" className="m-agent-status">
        {copied
          ? `Paste it into ${agent} to get started.`
          : manual
            ? 'Copy the selected prompt below and paste it into your agent.'
            : 'Builds the desktop app from source.'}
      </p>
      {manual && (
        <textarea
          ref={promptField}
          aria-label="Installation prompt"
          readOnly
          value={text}
          rows={5}
        />
      )}
      <a className="m-agent-guide" href="/install.md">
        Read the installation guide <span>↗</span>
      </a>
    </section>
  );
}
