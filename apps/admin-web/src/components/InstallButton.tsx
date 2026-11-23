import React, { useEffect, useState } from "react";

interface BeforeInstallPromptEvent extends Event {
  prompt: () => Promise<void>;
  userChoice: Promise<{ outcome: "accepted" | "dismissed"; platform: string }>;
}

/**
 * Chrome/Edge masaüstü + Android PWA kurulum düğmesi. Tarayıcı kurulum
 * istemini yalnız belirli koşullarda (manifest, service worker, HTTPS) verir.
 */
export default function InstallButton() {
  const [deferred, setDeferred] = useState<BeforeInstallPromptEvent | null>(null);
  const [installed, setInstalled] = useState(false);

  useEffect(() => {
    const onPrompt = (e: Event) => {
      e.preventDefault();
      setDeferred(e as BeforeInstallPromptEvent);
    };
    const onInstalled = () => {
      setInstalled(true);
      setDeferred(null);
    };
    window.addEventListener("beforeinstallprompt", onPrompt);
    window.addEventListener("appinstalled", onInstalled);
    if (window.matchMedia?.("(display-mode: standalone)").matches) setInstalled(true);
    return () => {
      window.removeEventListener("beforeinstallprompt", onPrompt);
      window.removeEventListener("appinstalled", onInstalled);
    };
  }, []);

  if (installed || !deferred) return null;

  return (
    <div className="p-3 border-t border-slate-800">
      <button
        onClick={async () => {
          try {
            await deferred.prompt();
            await deferred.userChoice;
          } finally {
            setDeferred(null);
          }
        }}
        className="w-full text-left text-xs text-slate-400 hover:text-brand-500"
        title="Vardrop panelini masaüstü uygulaması olarak kur"
      >
        ⤓ Uygulamayı yükle
      </button>
    </div>
  );
}
