"use client";

import { usePolicyVersionBuilder } from "../lib/policy-builder/use-policy-version-builder.ts";
import { PolicyCandidateReview } from "./policy-candidate-review.tsx";
import { PolicyCatalogue } from "./policy-catalogue.tsx";
import { PolicyField, PolicyRuleEditor } from "./policy-rule-controls.tsx";

export function PolicyVersionWorkspace({
  csrfToken,
  onSessionExpired,
}: {
  readonly csrfToken: string;
  readonly onSessionExpired: () => void;
}) {
  const model = usePolicyVersionBuilder(csrfToken, onSessionExpired);
  const busy = model.operation !== null;
  const locked = busy || model.attempt !== null;
  const nextFamily =
    model.prepared?.identity.familyId ??
    (model.familyMode === "existing" ? model.familyId : undefined);

  return (
    <section className="policy-workspace">
      <header className="view-header policy-view-header">
        <div>
          <p className="eyebrow">Authorization policy</p>
          <h1>Policy builder</h1>
        </div>
        <div className="policy-header-status">
          <span className="classification-badge">Versioned policies</span>
          <span>SpiceDB 1.54.0 subset</span>
        </div>
      </header>
      <PolicyCatalogue
        selectedFamilyId={model.familyMode === "existing" ? model.familyId : undefined}
        selectionDisabled={locked}
        refreshKey={model.published?.publication.record_id}
        onSelectFamily={model.selectExistingFamily}
        onSessionExpired={onSessionExpired}
      />
      <div className="policy-builder-shell">
        <div className="policy-authoring-pane">
          <fieldset disabled={locked} style={{ border: 0, padding: 0, margin: 0, minWidth: 0 }}>
            <legend className="sr-only">Policy version draft</legend>
            <section className="policy-section" aria-labelledby="policy-identity-heading">
              <div className="section-heading-row">
                <div>
                  <p className="section-index">01</p>
                  <h2 id="policy-identity-heading">Policy family</h2>
                </div>
                <span>Server-assigned version</span>
              </div>
              <div className="policy-field-grid">
                <label className="policy-field policy-select-field">
                  <span>Family selection</span>
                  <select
                    value={model.familyMode}
                    onChange={(event) =>
                      model.selectFamilyMode(event.target.value as "new" | "existing")
                    }
                  >
                    <option value="new">New policy family</option>
                    <option value="existing">Existing policy family</option>
                  </select>
                </label>
                {model.familyMode === "existing" ? (
                  <PolicyField
                    label="Policy family ID"
                    value={model.familyId}
                    onChange={model.selectFamilyId}
                  />
                ) : null}
              </div>
            </section>
            <PolicyRuleEditor builder={model.builder} update={model.update} />
          </fieldset>
          <section
            className="policy-section policy-submit-section"
            aria-labelledby="compile-heading"
          >
            <div className="section-heading-row">
              <div>
                <p className="section-index">03</p>
                <h2 id="compile-heading">Compile version</h2>
              </div>
              <span>Candidate lifetime</span>
            </div>
            <div className="compile-controls">
              <label>
                <span>Valid for seconds</span>
                <input
                  inputMode="numeric"
                  value={model.builder.validForSeconds}
                  disabled={locked}
                  onChange={(event) => model.update("validForSeconds", event.target.value)}
                />
              </label>
              <button
                className="primary-button compile-policy-button"
                type="button"
                disabled={busy || model.prepared !== null}
                onClick={() => void model.compile()}
              >
                {model.operation === "compile"
                  ? "Compiling..."
                  : model.prepared
                    ? "Version compiled"
                    : model.attempt
                      ? "Retry compile"
                      : "Compile version"}
              </button>
            </div>
            {model.attempt && !model.prepared ? (
              <div className="candidate-records">
                <span>Candidate ID</span>
                <code>{model.attempt.draft.candidateId}</code>
                <button
                  className="secondary-button"
                  type="button"
                  disabled={busy}
                  onClick={() => void model.refresh()}
                >
                  Refresh candidate
                </button>
              </div>
            ) : null}
            {model.error ? (
              <p className="form-error editor-message" role="alert">
                {model.error}
              </p>
            ) : null}
            {model.uncertain ? (
              <p className="candidate-stale" role="status">
                Outcome unconfirmed. This candidate is retained for retry.
              </p>
            ) : null}
            {model.notice ? (
              <p className="form-notice editor-message" role="status">
                {model.notice}
              </p>
            ) : null}
            {model.attempt ? (
              <div className="candidate-actions">
                <button
                  className="secondary-button"
                  type="button"
                  disabled={busy || model.uncertain}
                  onClick={() => model.startDraft(nextFamily)}
                >
                  {model.prepared ? "Draft next version" : "Edit draft"}
                </button>
                <button
                  className="secondary-button"
                  type="button"
                  disabled={busy || model.uncertain}
                  onClick={() => model.startDraft()}
                >
                  New policy family
                </button>
              </div>
            ) : null}
          </section>
        </div>
        <PolicyCandidateReview
          candidate={model.prepared?.candidate ?? null}
          activePolicy={model.published?.publication ?? null}
          {...(model.prepared ? { identity: model.prepared.identity } : {})}
          stale={false}
          busy={busy}
          publishing={model.operation === "publish"}
          onActivate={() => void model.publish()}
          onRefresh={() => void model.refresh()}
        />
      </div>
    </section>
  );
}
