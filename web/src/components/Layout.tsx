import { NavLink, Outlet } from 'react-router';

import { ServerStatus } from './ServerStatus';

const LINKS = [
  { to: '/play', label: 'Jouer' },
  { to: '/analysis', label: 'Analyse' },
  { to: '/training', label: 'Entraînement' },
  { to: '/arena', label: 'Arène' },
] as const;

/** Three bone squares and one signal square. */
function Logo() {
  return (
    <span aria-hidden className="grid size-6 grid-cols-2 gap-0.5">
      <span className="rounded-[3px] bg-bone" />
      <span className="rounded-[3px] bg-bone" />
      <span className="rounded-[3px] bg-bone" />
      <span className="rounded-[3px] bg-signal" />
    </span>
  );
}

export function Layout() {
  return (
    <div className="flex min-h-full flex-col">
      <header className="sticky top-0 z-20 border-b border-line bg-ink-950/85 backdrop-blur">
        <div className="mx-auto flex max-w-[1360px] flex-wrap items-center gap-x-8 gap-y-3 px-4 py-3 sm:px-6">
          <NavLink to="/play" className="flex items-center gap-2.5">
            <Logo />
            <span className="text-[17px] font-semibold tracking-tight">chess-bot</span>
          </NavLink>
          <nav className="flex gap-1">
            {LINKS.map(({ to, label }) => (
              <NavLink
                key={to}
                to={to}
                className={({ isActive }) =>
                  `rounded-xl px-3.5 py-2 text-sm transition-colors ${
                    isActive ? 'bg-signal/10 font-medium text-signal' : 'text-mute hover:text-bone'
                  }`
                }
              >
                {label}
              </NavLink>
            ))}
          </nav>
          <div className="ml-auto">
            <ServerStatus />
          </div>
        </div>
      </header>
      <main className="mx-auto w-full max-w-[1360px] flex-1 px-4 py-6 sm:px-6 sm:py-8">
        <Outlet />
      </main>
    </div>
  );
}
