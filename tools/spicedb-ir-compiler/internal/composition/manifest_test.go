package composition

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestManifestStrictContract(t *testing.T) {
	valid := `{"format_version":1,"packages":[{"version_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","schema_path":"s.zed","config_path":"c.json","expected_policy_ir_path":"p.pb"}]}`
	legacy := strings.Replace(valid, `"version_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"`, `"legacy":true`, 1)
	for _, document := range []string{valid, legacy} {
		if _, err := ParseManifest([]byte(document)); err != nil {
			t.Fatal(err)
		}
	}
	for name, document := range map[string]string{
		"unknown":            strings.Replace(valid, `"format_version":1`, `"format_version":1,"extra":true`, 1),
		"case":               strings.Replace(valid, "format_version", "Format_Version", 1),
		"duplicate":          strings.Replace(valid, `"format_version":1`, `"format_version":1,"format_version":1`, 1),
		"nested duplicate":   strings.Replace(valid, `"schema_path":"s.zed"`, `"schema_path":"s.zed","schema_path":"s.zed"`, 1),
		"null":               strings.Replace(valid, `"s.zed"`, "null", 1),
		"wrong version":      strings.Replace(valid, `"format_version":1`, `"format_version":2`, 1),
		"string version":     strings.Replace(valid, `"format_version":1`, `"format_version":"1"`, 1),
		"fractional version": strings.Replace(valid, `"format_version":1`, `"format_version":1.0`, 1),
		"empty":              `{"format_version":1,"packages":[]}`,
		"empty path":         strings.Replace(valid, `"s.zed"`, `""`, 1),
		"legacy false":       strings.Replace(legacy, "true", "false", 1),
		"both identities":    strings.Replace(valid, `"schema_path"`, `"legacy":true,"schema_path"`, 1),
		"uppercase ID":       strings.Replace(valid, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", 1),
		"second document":    valid + valid,
		"oversized":          strings.Repeat(" ", MaxManifestBytes) + valid,
	} {
		t.Run(name, func(t *testing.T) {
			if _, err := ParseManifest([]byte(document)); err == nil {
				t.Fatal("invalid manifest accepted")
			}
		})
	}
}

func TestManifestRelativePathsAndInputKinds(t *testing.T) {
	dir := t.TempDir()
	entry := PackageInput{VersionID: versionA, SchemaPath: "schema.zed", ConfigPath: "config.json", ExpectedPolicyIRPath: "policy.pb"}
	data, _ := json.Marshal(Manifest{FormatVersion: 1, Packages: []PackageInput{entry}})
	path := filepath.Join(dir, "manifest.json")
	writeTest(t, path, data)
	manifest, err := LoadManifest(path)
	if err != nil || manifest.Packages[0].SchemaPath != filepath.Join(dir, "schema.zed") {
		t.Fatalf("relative resolution: %v", err)
	}
	link := filepath.Join(dir, "link")
	if err := os.Symlink(path, link); err != nil {
		t.Fatal(err)
	}
	for _, invalid := range []string{link, dir, filepath.Join(dir, "absent")} {
		if _, err := LoadManifest(invalid); err == nil {
			t.Fatal("nonregular input accepted")
		}
	}
}
