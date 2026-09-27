"use client";
import {
  DIRECT_RELATION_OPTIONS,
  POLICY_LEAF_OPTIONS,
  POLICY_MODE_OPTIONS,
  type PolicyBuilderState,
  type PolicyLeafSelection,
} from "../lib/policy-builder/index.ts";
import {
  CAVEAT_CATALOG,
  type CaveatSelection,
  type DirectRelation,
} from "../lib/policy-draft/index.ts";
export function PolicyRuleEditor({
  builder,
  update,
}: {
  readonly builder: PolicyBuilderState;
  readonly update: <K extends keyof PolicyBuilderState>(
    field: K,
    value: PolicyBuilderState[K],
  ) => void;
}) {
  const selectedMode = POLICY_MODE_OPTIONS.find((option) => option.value === builder.mode);
  return (
    <section className="policy-section" aria-labelledby="rule-family-heading">
      <div className="section-heading-row">
        <div>
          <p className="section-index">02</p>
          <h2 id="rule-family-heading">Rule family</h2>
        </div>
        <span>{selectedMode?.description}</span>
      </div>
      <div className="policy-mode-selector" role="tablist" aria-label="Rule family">
        {POLICY_MODE_OPTIONS.map((option) => (
          <button
            key={option.value}
            type="button"
            role="tab"
            aria-selected={builder.mode === option.value}
            data-active={builder.mode === option.value}
            onClick={() => update("mode", option.value)}
          >
            {option.label}
          </button>
        ))}
      </div>

      <RuleConfiguration builder={builder} update={update} />
      <RuleDiagram builder={builder} />
    </section>
  );
}
function RuleConfiguration({
  builder,
  update,
}: {
  readonly builder: PolicyBuilderState;
  readonly update: <K extends keyof PolicyBuilderState>(
    field: K,
    value: PolicyBuilderState[K],
  ) => void;
}) {
  switch (builder.mode) {
    case "direct":
      return (
        <div className="rule-control-grid single-rule-control">
          <SelectField
            label="Agent relation"
            value={builder.directRelation}
            options={DIRECT_RELATION_OPTIONS.map((value) => ({
              value,
              label: relationLabel(value),
            }))}
            onChange={(value) => update("directRelation", value as DirectRelation)}
          />
        </div>
      );
    case "team":
      return (
        <div className="fixed-rule-note">
          <strong>team_viewer</strong>
          <span>The related team must contain the requesting agent as a member.</span>
        </div>
      );
    case "alternative":
      return (
        <div className="rule-control-grid">
          <SelectField
            label="First path"
            value={builder.alternativeLeft}
            options={POLICY_LEAF_OPTIONS}
            onChange={(value) => update("alternativeLeft", value as PolicyLeafSelection)}
          />
          <SelectField
            label="Second path"
            value={builder.alternativeRight}
            options={POLICY_LEAF_OPTIONS}
            onChange={(value) => update("alternativeRight", value as PolicyLeafSelection)}
          />
        </div>
      );
    case "dual_control":
      return (
        <div className="rule-control-grid">
          <SelectField
            label="Required relation A"
            value={builder.dualLeft}
            options={DIRECT_RELATION_OPTIONS.map((value) => ({
              value,
              label: relationLabel(value),
            }))}
            onChange={(value) => update("dualLeft", value as DirectRelation)}
          />
          <SelectField
            label="Required relation B"
            value={builder.dualRight}
            options={DIRECT_RELATION_OPTIONS.map((value) => ({
              value,
              label: relationLabel(value),
            }))}
            onChange={(value) => update("dualRight", value as DirectRelation)}
          />
        </div>
      );
    case "caveat": {
      const selectedCaveat = CAVEAT_CATALOG.find(
        (entry) => entry.template === builder.caveatTemplate,
      );
      const bindsClaims =
        builder.caveatTemplate === "task_purpose_scope" ||
        builder.caveatTemplate === "reconciliation_scope";
      return (
        <div className="caveat-controls">
          <div className="rule-control-grid">
            <SelectField
              label="Agent relation"
              value={builder.caveatRelation}
              options={DIRECT_RELATION_OPTIONS.map((value) => ({
                value,
                label: relationLabel(value),
              }))}
              onChange={(value) => update("caveatRelation", value as DirectRelation)}
            />
            <SelectField
              label="Caveat template"
              value={builder.caveatTemplate}
              options={CAVEAT_CATALOG.map((entry) => ({
                value: entry.template,
                label: entry.label,
              }))}
              onChange={(value) => update("caveatTemplate", value as CaveatSelection["template"])}
            />
          </div>
          {selectedCaveat ? (
            <div className="caveat-definition">
              <p>{selectedCaveat.description}</p>
              <ul>
                {selectedCaveat.parameters.map((parameter) => (
                  <li key={parameter.name}>
                    <code>{parameter.name}</code>
                    <span>{sourceLabel(parameter.source)}</span>
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
          {bindsClaims ? (
            <div className="policy-field-grid caveat-claim-fields">
              <PolicyField
                label="Task ID"
                value={builder.taskId}
                onChange={(value) => update("taskId", value)}
              />
              <PolicyField
                label="Purpose"
                value={builder.purpose}
                onChange={(value) => update("purpose", value)}
              />
              <PolicyField
                label="Resource scope"
                value={builder.resourceScope}
                onChange={(value) => update("resourceScope", value)}
              />
            </div>
          ) : null}
        </div>
      );
    }
  }
}

function RuleDiagram({ builder }: { readonly builder: PolicyBuilderState }) {
  const leaves = diagramLeaves(builder);
  const operator =
    builder.mode === "alternative" ? "ANY" : builder.mode === "dual_control" ? "ALL" : null;
  return (
    <fieldset className="rule-diagram">
      <legend className="sr-only">Policy rule preview</legend>
      <div className="rule-diagram-header">
        <span>Visual rule</span>
        <code>permission query</code>
      </div>
      <div className="rule-diagram-body">
        {operator ? <span className="rule-operator">{operator}</span> : null}
        <div className="rule-leaf-row">
          {leaves.map((leaf) => (
            <div className="rule-leaf" key={leaf.key}>
              <span>{leaf.type}</span>
              <strong>{leaf.label}</strong>
              {leaf.detail ? <small>{leaf.detail}</small> : null}
            </div>
          ))}
        </div>
      </div>
    </fieldset>
  );
}

export function PolicyField({
  label,
  value,
  inputMode,
  onChange,
}: {
  readonly label: string;
  readonly value: string;
  readonly inputMode?: "numeric";
  readonly onChange: (value: string) => void;
}) {
  return (
    <label className="policy-field">
      <span>{label}</span>
      <input
        inputMode={inputMode}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}

function SelectField({
  label,
  value,
  options,
  onChange,
}: {
  readonly label: string;
  readonly value: string;
  readonly options: readonly { readonly value: string; readonly label: string }[];
  readonly onChange: (value: string) => void;
}) {
  return (
    <label className="policy-field policy-select-field">
      <span>{label}</span>
      <select value={value} onChange={(event) => onChange(event.target.value)}>
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </label>
  );
}

interface DiagramLeaf {
  readonly key: string;
  readonly type: string;
  readonly label: string;
  readonly detail?: string;
}

function diagramLeaves(builder: PolicyBuilderState): readonly DiagramLeaf[] {
  switch (builder.mode) {
    case "direct":
      return [directDiagramLeaf(builder.directRelation, "direct")];
    case "team":
      return [{ key: "team", type: "subject set", label: "team_viewer", detail: "team#member" }];
    case "alternative":
      return [
        selectionDiagramLeaf(builder.alternativeLeft, "alternative-left"),
        selectionDiagramLeaf(builder.alternativeRight, "alternative-right"),
      ];
    case "dual_control":
      return [
        directDiagramLeaf(builder.dualLeft, "dual-left"),
        directDiagramLeaf(builder.dualRight, "dual-right"),
      ];
    case "caveat": {
      const caveatLabel = CAVEAT_CATALOG.find(
        (entry) => entry.template === builder.caveatTemplate,
      )?.label;
      return [
        {
          ...directDiagramLeaf(builder.caveatRelation, "caveat"),
          ...(caveatLabel ? { detail: caveatLabel } : {}),
        },
      ];
    }
  }
}

function selectionDiagramLeaf(selection: PolicyLeafSelection, key: string): DiagramLeaf {
  if (selection === "team_viewer") {
    return { key, type: "subject set", label: "team_viewer", detail: "team#member" };
  }
  if (selection === "organization_audit") {
    return { key, type: "delegation", label: "organization", detail: "organization->audit" };
  }
  return directDiagramLeaf(selection, key);
}

function directDiagramLeaf(relation: DirectRelation, key: string): DiagramLeaf {
  return { key, type: "direct relation", label: relation };
}

function relationLabel(relation: DirectRelation): string {
  return relation.charAt(0).toUpperCase() + relation.slice(1);
}

function sourceLabel(source: string): string {
  switch (source) {
    case "CAPABILITY_CLAIM":
      return "Capability claim";
    case "RELATIONSHIP_CONTEXT":
      return "Committed relationship";
    case "REQUEST_CONTEXT":
      return "Request context";
    default:
      return source;
  }
}
