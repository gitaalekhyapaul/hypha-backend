// Package composition validates and composes owner-supplied policy packages offline.
// Approval signatures, installed state and snapshot provenance belong to the owner.
package composition

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"path/filepath"
	"strings"
	"unicode/utf8"
)

const (
	MaxPackages        = 32
	MaxManifestBytes   = 64 * 1024
	MaxSchemaBytes     = 1024 * 1024
	MaxReflectionBytes = 1024 * 1024
	MaxTransportBytes  = 2 * 1024 * 1024
	MaxConfigBytes     = 64 * 1024
	MaxPolicyIRBytes   = 1024 * 1024
)

var (
	ErrManifest    = errors.New("invalid composition manifest")
	ErrInput       = errors.New("invalid composition input")
	ErrPolicy      = errors.New("policy package validation failed")
	ErrOwnership   = errors.New("policy package ownership conflict")
	ErrComposition = errors.New("schema composition validation failed")
	ErrLimit       = errors.New("composition limit exceeded")
)

// Manifest paths are absolute after LoadManifest, or supplied directly by an owner caller.
type Manifest struct {
	FormatVersion uint32         `json:"format_version"`
	Packages      []PackageInput `json:"packages"`
}

// PackageInput selects a namespaced version or an exact historical legacy package.
type PackageInput struct {
	VersionID            string `json:"version_id,omitempty"`
	Legacy               bool   `json:"legacy,omitempty"`
	SchemaPath           string `json:"schema_path"`
	ConfigPath           string `json:"config_path"`
	ExpectedPolicyIRPath string `json:"expected_policy_ir_path"`
}

// LoadManifest rejects unknown, duplicate, null and incorrectly cased JSON fields.
// Relative paths resolve against the manifest directory, never the working directory.
func LoadManifest(path string) (Manifest, error) {
	data, err := readBounded(path, MaxManifestBytes)
	if err != nil {
		return Manifest{}, err
	}
	manifest, err := ParseManifest(data)
	if err != nil {
		return Manifest{}, err
	}
	base, err := filepath.Abs(filepath.Dir(path))
	if err != nil {
		return Manifest{}, ErrInput
	}
	for i := range manifest.Packages {
		entry := &manifest.Packages[i]
		for _, path := range []*string{&entry.SchemaPath, &entry.ConfigPath, &entry.ExpectedPolicyIRPath} {
			if !filepath.IsAbs(*path) {
				*path = filepath.Join(base, *path)
			}
		}
	}
	return manifest, nil
}

func ParseManifest(data []byte) (Manifest, error) {
	if len(data) > MaxManifestBytes {
		return Manifest{}, ErrLimit
	}
	if !utf8.Valid(data) || validateJSON(data) != nil {
		return Manifest{}, ErrManifest
	}
	root, err := objectFields(data, "format_version", "packages")
	if err != nil || len(root) != 2 {
		return Manifest{}, ErrManifest
	}
	var manifest Manifest
	if json.Unmarshal(root["format_version"], &manifest.FormatVersion) != nil || manifest.FormatVersion != 1 {
		return Manifest{}, ErrManifest
	}
	var entries []json.RawMessage
	if json.Unmarshal(root["packages"], &entries) != nil || len(entries) == 0 || len(entries) > MaxPackages+1 {
		return Manifest{}, ErrManifest
	}
	for _, raw := range entries {
		fields, err := objectFields(raw, "version_id", "legacy", "schema_path", "config_path", "expected_policy_ir_path")
		if err != nil || len(fields) != 4 {
			return Manifest{}, ErrManifest
		}
		var entry PackageInput
		if json.Unmarshal(raw, &entry) != nil {
			return Manifest{}, ErrManifest
		}
		_, versionPresent := fields["version_id"]
		_, legacyPresent := fields["legacy"]
		if versionPresent == legacyPresent || legacyPresent && !entry.Legacy {
			return Manifest{}, ErrManifest
		}
		manifest.Packages = append(manifest.Packages, entry)
	}
	if err := manifest.validate(); err != nil {
		return Manifest{}, err
	}
	return manifest, nil
}

func (m Manifest) validate() error {
	if m.FormatVersion != 1 || len(m.Packages) == 0 || len(m.Packages) > MaxPackages+1 {
		return ErrManifest
	}
	versions := map[string]bool{}
	legacyCount := 0
	for _, entry := range m.Packages {
		if entry.Legacy {
			legacyCount++
			if entry.VersionID != "" || legacyCount > 1 {
				return ErrManifest
			}
		} else {
			if !validVersionID(entry.VersionID) || versions[entry.VersionID] {
				return ErrOwnership
			}
			versions[entry.VersionID] = true
		}
		for _, path := range []string{entry.SchemaPath, entry.ConfigPath, entry.ExpectedPolicyIRPath} {
			if path == "" || len(path) > 4096 || strings.ContainsRune(path, 0) {
				return ErrManifest
			}
		}
	}
	if len(versions) > MaxPackages {
		return ErrLimit
	}
	return nil
}

func validVersionID(value string) bool {
	if len(value) != 32 {
		return false
	}
	for _, c := range value {
		if !(c >= '0' && c <= '9' || c >= 'a' && c <= 'f') {
			return false
		}
	}
	return true
}

func objectFields(data []byte, allowed ...string) (map[string]json.RawMessage, error) {
	var fields map[string]json.RawMessage
	if json.Unmarshal(data, &fields) != nil || fields == nil {
		return nil, ErrManifest
	}
	for key, value := range fields {
		found := false
		for _, name := range allowed {
			found = found || name == key
		}
		if !found || bytes.Equal(bytes.TrimSpace(value), []byte("null")) {
			return nil, ErrManifest
		}
	}
	return fields, nil
}

func validateJSON(data []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	if err := walkJSON(decoder, 0); err != nil {
		return err
	}
	if _, err := decoder.Token(); err != io.EOF {
		return ErrManifest
	}
	return nil
}

func walkJSON(decoder *json.Decoder, depth int) error {
	if depth > 32 {
		return ErrLimit
	}
	token, err := decoder.Token()
	if err != nil {
		return ErrManifest
	}
	delimiter, ok := token.(json.Delim)
	if !ok {
		return nil
	}
	if delimiter != '{' && delimiter != '[' {
		return ErrManifest
	}
	keys := map[string]bool{}
	for decoder.More() {
		if delimiter == '{' {
			token, err := decoder.Token()
			key, ok := token.(string)
			if err != nil || !ok || keys[key] {
				return ErrManifest
			}
			keys[key] = true
		}
		if err := walkJSON(decoder, depth+1); err != nil {
			return err
		}
	}
	_, err = decoder.Token()
	return err
}

func readBounded(path string, limit int) ([]byte, error) {
	info, err := os.Lstat(path)
	if err != nil || !info.Mode().IsRegular() {
		return nil, ErrInput
	}
	if info.Size() > int64(limit) {
		return nil, ErrLimit
	}
	file, err := os.Open(path)
	if err != nil {
		return nil, ErrInput
	}
	defer file.Close()
	opened, err := file.Stat()
	if err != nil || !os.SameFile(info, opened) {
		return nil, ErrInput
	}
	data, err := io.ReadAll(io.LimitReader(file, int64(limit)+1))
	if err != nil {
		return nil, ErrInput
	}
	if len(data) > limit {
		return nil, ErrLimit
	}
	return data, nil
}
