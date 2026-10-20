/** FlowLedger brand mark — ledger bars + flow pulse. */
export function BrandMark({ className = "h-5 w-5" }: { className?: string }) {
  return (
    <svg
      className={className}
      viewBox="0 0 32 32"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden
    >
      <rect x="4" y="7" width="24" height="3.2" rx="1.2" fill="currentColor" opacity="0.95" />
      <rect x="4" y="14.4" width="24" height="3.2" rx="1.2" fill="currentColor" opacity="0.75" />
      <rect x="4" y="21.8" width="24" height="3.2" rx="1.2" fill="currentColor" opacity="0.55" />
      <path
        d="M5 16.5 C9 10.5, 12 22.5, 16 16.5 C20 10.5, 23 22.5, 27 16.5"
        stroke="currentColor"
        strokeWidth="2.2"
        strokeLinecap="round"
        strokeLinejoin="round"
        opacity="1"
      />
    </svg>
  );
}

export function BrandWordmark({ className = "" }: { className?: string }) {
  return (
    <div className={className}>
      <div className="font-bold leading-5 tracking-tight">
        flowledger<span className="text-accent">.</span>
      </div>
      <div className="tiny tracking-[0.12em]">DEVELOPER ECONOMICS</div>
    </div>
  );
}
