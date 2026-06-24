import {
  createContext,
  useContext,
  useState,
  useCallback,
  useEffect,
  ReactNode,
} from "react";
import { translations, languageMeta, Language, Translation } from "./translations";

interface I18nContextValue {
  lang: Language;
  t: Translation;
  setLang: (lang: Language) => void;
  /** 当前语言对应的 BCP 47 locale（如 "zh-CN"、"ja-JP"），用于 toLocaleTimeString 等 */
  locale: string;
}

const I18nContext = createContext<I18nContextValue | null>(null);

const STORAGE_KEY = "clipsync-lang";
const MANUAL_KEY = "clipsync-lang-manual";

const VALID_LANGS = Object.keys(languageMeta) as Language[];

function detectSystemLang(): Language {
  const langs = navigator.languages?.length
    ? navigator.languages
    : [navigator.language];
  for (const bl of langs) {
    const lower = bl.toLowerCase();
    for (const lang of VALID_LANGS) {
      if (lower.startsWith(lang)) return lang;
    }
  }
  return "en";
}

function getInitialLang(): Language {
  // 只有用户手动选过语言才读 localStorage
  const wasManual = localStorage.getItem(MANUAL_KEY);
  if (wasManual === "1") {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved && VALID_LANGS.includes(saved as Language)) return saved as Language;
  }
  // 跟随系统语言
  return detectSystemLang();
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Language>(getInitialLang);

  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);

  const setLang = useCallback((l: Language) => {
    setLangState(l);
    // 用户手动选择时才持久化
    localStorage.setItem(STORAGE_KEY, l);
    localStorage.setItem(MANUAL_KEY, "1");
  }, []);

  const value: I18nContextValue = {
    lang,
    t: translations[lang],
    setLang,
    locale: languageMeta[lang].locale,
  };

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nContextValue {
  const ctx = useContext(I18nContext);
  if (!ctx) {
    throw new Error("useI18n must be used within I18nProvider");
  }
  return ctx;
}
