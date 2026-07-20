package config

import (
	"strings"
	"testing"

	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
)

const validConfig = `{
  "config_version": 1,
  "limits": {
    "max_expression_nodes": 64,
    "max_caveat_nodes": 64,
    "max_path_depth": 4,
    "max_relationship_witnesses": 32,
    "max_merkle_depth": 32,
    "max_string_bytes": 256,
    "max_context_entries": 16,
    "max_total_private_input_bytes": 4096
  },
  "caveats": {
    "reconcile_scope": {
      "task_id": "CAPABILITY_CLAIM",
      "approved_amount": "RELATIONSHIP_CONTEXT",
      "requested_amount": "REQUEST_CONTEXT"
    }
  }
}`

func TestParseValidConfig(t *testing.T) {
	parsed, err := Parse([]byte(validConfig))
	if err != nil {
		t.Fatalf("Parse() error = %v", err)
	}

	if got, want := parsed.Limits().GetMaxExpressionNodes(), uint32(64); got != want {
		t.Errorf("max_expression_nodes = %d, want %d", got, want)
	}
	bindings := parsed.CaveatBindings()["reconcile_scope"]
	if got, want := bindings["task_id"], razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_CAPABILITY_CLAIM; got != want {
		t.Errorf("task_id source = %s, want %s", got, want)
	}
	if got, want := bindings["approved_amount"], razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_RELATIONSHIP_CONTEXT; got != want {
		t.Errorf("approved_amount source = %s, want %s", got, want)
	}
	if got, want := bindings["requested_amount"], razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_REQUEST_CONTEXT; got != want {
		t.Errorf("requested_amount source = %s, want %s", got, want)
	}

	bindings["task_id"] = razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_REQUEST_CONTEXT
	if got := parsed.CaveatBindings()["reconcile_scope"]["task_id"]; got != razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_CAPABILITY_CLAIM {
		t.Error("CaveatBindings() returned mutable internal state")
	}
}

func TestParseRejectsInvalidConfig(t *testing.T) {
	tests := []struct {
		name    string
		config  string
		message string
	}{
		{
			name:    "duplicate nested key",
			config:  strings.Replace(validConfig, `"task_id": "CAPABILITY_CLAIM"`, `"task_id": "CAPABILITY_CLAIM", "task_id": "REQUEST_CONTEXT"`, 1),
			message: `duplicate JSON key "task_id"`,
		},
		{
			name:    "unknown field",
			config:  strings.Replace(validConfig, `"config_version": 1,`, `"config_version": 1, "unexpected": true,`, 1),
			message: `unknown field "unexpected"`,
		},
		{
			name:    "trailing value",
			config:  validConfig + ` {}`,
			message: "compiler config must contain one JSON value",
		},
		{
			name:    "unsupported version",
			config:  strings.Replace(validConfig, `"config_version": 1`, `"config_version": 2`, 1),
			message: "unsupported compiler config version 2",
		},
		{
			name:    "zero limit",
			config:  strings.Replace(validConfig, `"max_path_depth": 4`, `"max_path_depth": 0`, 1),
			message: `limit "max_path_depth" must be greater than zero`,
		},
		{
			name:    "unknown source",
			config:  strings.Replace(validConfig, `"task_id": "CAPABILITY_CLAIM"`, `"task_id": "SESSION"`, 1),
			message: `unknown input source "SESSION"`,
		},
		{
			name:    "missing caveats object",
			config:  strings.Replace(validConfig, `"caveats": {`, `"not_caveats": {`, 1),
			message: `unknown field "not_caveats"`,
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			_, err := Parse([]byte(test.config))
			if err == nil {
				t.Fatal("Parse() error = nil")
			}
			if !strings.Contains(err.Error(), test.message) {
				t.Fatalf("Parse() error = %q, want substring %q", err, test.message)
			}
		})
	}
}
