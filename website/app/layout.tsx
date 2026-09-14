import type { Metadata } from "next";
import "./globals.css";
import "./hero.css";
import "./light.css";
import "./demo.css";
import "./analytics.css";
import Analytics from "./components/analytics";
export const metadata: Metadata = {
  title: "Schematlas — See how it all connects",
  description:
    "Explore database schemas and OpenAPI definitions in one local desktop workspace. Free, open source, and available for macOS, Windows, and Linux.",
  icons: { icon: "/favicon.svg", shortcut: "/favicon.svg" },
  openGraph: {
    title: "Schematlas — See how it all connects",
    description:
      "Your databases, APIs, and coding agents. One visual workspace, right on your desktop.",
    type: "website",
  },
};
export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <head>
        <meta name="color-scheme" content="light" />
      </head>
      <body>{children}<Analytics /></body>
    </html>
  );
}
