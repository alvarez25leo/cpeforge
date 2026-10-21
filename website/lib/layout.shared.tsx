import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';
import { Logo } from '@/components/logo';
import { appName, repoUrl } from './shared';

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: (
        <>
          <Logo className="size-6" />
          <span className="font-semibold">{appName}</span>
        </>
      ),
    },
    githubUrl: repoUrl,
    links: [
      { text: 'Documentación', url: '/docs', active: 'nested-url' },
      { text: 'crates.io', url: 'https://crates.io/crates/cpeforge', external: true },
      { text: 'docs.rs', url: 'https://docs.rs/cpeforge', external: true },
    ],
  };
}
