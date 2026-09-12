import type {ReactNode} from 'react';
import clsx from 'clsx';
import Link from '@docusaurus/Link';
import useBaseUrl from '@docusaurus/useBaseUrl';
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';
import FeatureHighlights from '@site/src/components/FeatureHighlights';

import styles from './index.module.css';

function HomepageHeader() {
  const dashboard = useBaseUrl('/readme-dashboard.png');
  return (
    <header className={clsx('hero hero--primary', styles.heroBanner)}>
      <div className="container">
        <div className={clsx(styles.heroText, 'text--center')}>
          <Heading as="h1" className="hero__title">
            Run Kubernetes VMs
            <br />
            like a platform.
          </Heading>
          <p className="hero__subtitle">
            Every KubeVirt shop ends up with the same pile of one-off
            scripts for create, snapshot, migrate, backup, and access
            control. Zorvia replaces that pile with one real backend — CLI,
            interactive TUI, and a signed-in web console, all driving the
            same API, all wired to real Kubernetes objects underneath. No
            fake dashboards, no dead buttons.
          </p>
          <div className={styles.buttons}>
            <Link
              className="button button--secondary button--lg"
              to="https://github.com/zyvorai/zorvia#quick-start">
              Try it now
            </Link>
            <Link
              className="button button--outline button--lg button--secondary"
              to="https://github.com/zyvorai/zorvia">
              View on GitHub
            </Link>
          </div>
        </div>
      </div>
      <div className={styles.heroMediaWrap}>
        <img
          className={styles.heroMedia}
          src={dashboard}
          alt="Zorvia dashboard"
        />
        <p className={styles.heroMediaCaption}>
          The Zorvia web console — CLI, TUI, and web all driving the same
          API.
        </p>
      </div>
    </header>
  );
}

function ProblemStatement() {
  return (
    <section className={styles.problem}>
      <div className="container">
        <div className="row">
          <div className="col col--8 col--offset-2 text--center">
            <Heading as="h2" className={styles.sectionHeading}>
              Stop hand-writing VM CRDs
            </Heading>
            <p>
              KubeVirt gives you the primitives — VirtualMachine,
              VirtualMachineInstance, DataVolume — but not an operator
              experience. Teams end up writing their own scripts for the
              same handful of operations, over and over, with no shared
              backend and no consistent access control.
            </p>
            <p>
              Zorvia is that backend: one real API behind the CLI, the
              interactive TUI, and the web console, all reading and
              writing real Kubernetes objects — not a parallel state store
              that can drift from the cluster.
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}

function TrustBand() {
  return (
    <section className={styles.trust}>
      <div className="container">
        <div className={styles.trustGrid}>
          <div>
            <Heading as="h3" className={styles.sectionHeading}>
              Apache-2.0, KubeVirt-native
            </Heading>
            <p>
              Apache-2.0 licensed, written in Rust. Zorvia doesn't
              reimplement VM execution — it's a real backend and UX layer
              on top of KubeVirt's own CRDs, so what you see in the CLI,
              TUI, or web console is what's actually running in the
              cluster.
            </p>
            <Link to="https://github.com/zyvorai/zorvia#readme">
              Read the full README →
            </Link>
          </div>
          <div className={styles.trustBadges}>
            <img
              src="https://github.com/zyvorai/zorvia/workflows/CI/badge.svg"
              alt="CI status"
            />
            <img
              src="https://img.shields.io/badge/License-Apache%202.0-blue.svg"
              alt="Apache 2.0 license"
            />
          </div>
        </div>
      </div>
    </section>
  );
}

function EnterpriseCTA() {
  return (
    <section className={styles.enterprise}>
      <div className="container text--center">
        <Heading as="h2" className={styles.sectionHeading}>
          Want to talk it through?
        </Heading>
        <p className={styles.enterpriseCopy}>
          Zorvia is Apache-2.0 and free to run in production. Reach out if
          you want to talk through a KubeVirt rollout or what Zorvia looks
          like at fleet scale.
        </p>
        <Link
          className="button button--primary button--lg"
          to="mailto:sales@zyvor.dev">
          Talk to sales
        </Link>
      </div>
    </section>
  );
}

export default function Home(): ReactNode {
  return (
    <Layout
      title="Zorvia — run Kubernetes VMs like a platform"
      description="Zorvia replaces one-off KubeVirt scripts with one real backend — CLI, interactive TUI, and a signed-in web console, all driving the same API.">
      <HomepageHeader />
      <main>
        <ProblemStatement />
        <FeatureHighlights />
        <TrustBand />
        <EnterpriseCTA />
      </main>
    </Layout>
  );
}
