import {
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";
import { Check, ChevronDown, Search } from "lucide-react";

export type SearchableOption = {
  value: string;
  label: string;
  hint?: string;
};

type Props = {
  options: SearchableOption[];
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  emptyLabel?: string;
  searchPlaceholder?: string;
  className?: string;
  disabled?: boolean;
};

export function SearchableSelect({
  options,
  value,
  onChange,
  placeholder = "Select…",
  emptyLabel = "No options",
  searchPlaceholder = "Search…",
  className = "",
  disabled = false,
}: Props) {
  const listId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);

  const selected = options.find((o) => o.value === value);
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return options;
    return options.filter((o) => {
      const hay = `${o.label} ${o.hint ?? ""}`.toLowerCase();
      return hay.includes(q);
    });
  }, [options, query]);

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setActiveIndex(0);
    const t = window.setTimeout(() => searchRef.current?.focus(), 0);
    return () => window.clearTimeout(t);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    function onDoc(e: MouseEvent) {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  useEffect(() => {
    setActiveIndex(0);
  }, [query]);

  function pick(next: string) {
    onChange(next);
    setOpen(false);
  }

  function onTriggerKey(e: KeyboardEvent) {
    if (disabled) return;
    if (e.key === "Enter" || e.key === " " || e.key === "ArrowDown") {
      e.preventDefault();
      setOpen(true);
    }
  }

  function onSearchKey(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActiveIndex((i) => Math.min(i + 1, Math.max(filtered.length - 1, 0)));
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIndex((i) => Math.max(i - 1, 0));
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      const opt = filtered[activeIndex];
      if (opt) pick(opt.value);
    }
  }

  return (
    <div ref={rootRef} className={`searchable-select ${className}`.trim()}>
      <button
        type="button"
        className="searchable-select-trigger"
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        onClick={() => !disabled && setOpen((v) => !v)}
        onKeyDown={onTriggerKey}
      >
        <span className="searchable-select-value truncate">
          {selected?.label || (options.length === 0 ? emptyLabel : placeholder)}
        </span>
        <ChevronDown className={`h-4 w-4 shrink-0 opacity-70 ${open ? "rotate-180" : ""}`} />
      </button>

      {open && (
        <div className="searchable-select-menu" role="presentation">
          <div className="searchable-select-search">
            <Search className="h-3.5 w-3.5 shrink-0 opacity-60" />
            <input
              ref={searchRef}
              className="searchable-select-input"
              value={query}
              placeholder={searchPlaceholder}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={onSearchKey}
              aria-autocomplete="list"
              aria-controls={listId}
            />
          </div>
          <ul id={listId} className="searchable-select-list" role="listbox">
            {filtered.length === 0 && (
              <li className="searchable-select-empty">No matches</li>
            )}
            {filtered.map((opt, i) => {
              const isSelected = opt.value === value;
              const isActive = i === activeIndex;
              return (
                <li key={opt.value} role="option" aria-selected={isSelected}>
                  <button
                    type="button"
                    className={`searchable-select-option ${isSelected ? "selected" : ""} ${
                      isActive ? "active" : ""
                    }`}
                    onMouseEnter={() => setActiveIndex(i)}
                    onClick={() => pick(opt.value)}
                  >
                    <span className="min-w-0 flex-1 truncate text-left">
                      <span className="block truncate font-semibold">{opt.label}</span>
                      {opt.hint && (
                        <span className="tiny mt-0.5 block truncate">{opt.hint}</span>
                      )}
                    </span>
                    {isSelected && <Check className="h-3.5 w-3.5 shrink-0 text-accent" />}
                  </button>
                </li>
              );
            })}
          </ul>
        </div>
      )}
    </div>
  );
}
