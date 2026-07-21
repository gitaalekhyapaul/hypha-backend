package equivalencefixture

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"

	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/config"
	"google.golang.org/protobuf/proto"
)

const (
	maxFixtureBytes int64 = 1_048_576
	maxSchemaBytes  int64 = 1_048_576
	maxConfigBytes  int64 = 1_048_576
	maxPolicyBytes  int64 = 4_194_304
)

// FindRepositoryRoot locates the canonical repository boundary from the current
// directory or from the fixture location.
func FindRepositoryRoot(fixturePath string) (string, error) {
	cwd, err := os.Getwd()
	if err != nil {
		return "", fixtureError(CodeRepositoryRoot, "cannot inspect the current directory")
	}

	candidates := make([]string, 0, 2)
	if fixturePath != "" {
		candidate := fixturePath
		if !filepath.IsAbs(candidate) {
			candidate = filepath.Join(cwd, candidate)
		}
		candidates = append(candidates, filepath.Dir(filepath.Clean(candidate)))
	}
	candidates = append(candidates, cwd)

	visited := map[string]struct{}{}
	for _, candidate := range candidates {
		for directory := candidate; ; directory = filepath.Dir(directory) {
			if _, seen := visited[directory]; !seen {
				visited[directory] = struct{}{}
				if hasRepositoryMarkers(directory) {
					canonical, err := filepath.EvalSymlinks(directory)
					if err != nil {
						return "", fixtureError(CodeRepositoryRoot, "cannot resolve the repository boundary")
					}
					return canonical, nil
				}
			}
			parent := filepath.Dir(directory)
			if parent == directory {
				break
			}
		}
	}
	return "", fixtureError(CodeRepositoryRoot, "repository markers were not found")
}

// Load parses and validates one fixture under repoRoot.
func Load(repoRoot, fixturePath string) (*Fixture, error) {
	canonicalRoot, err := filepath.EvalSymlinks(repoRoot)
	if err != nil {
		return nil, fixtureError(CodeRepositoryRoot, "cannot resolve the repository boundary")
	}
	fixtureFile, err := resolveConfinedFile(canonicalRoot, fixturePath, true)
	if err != nil {
		return nil, err
	}
	fixtureBytes, err := readFile(fixtureFile, "fixture", maxFixtureBytes)
	if err != nil {
		return nil, err
	}

	var document fixtureDocument
	if err := decodeStrictJSON(fixtureBytes, &document); err != nil {
		return nil, fixtureError(CodeJSON, err.Error())
	}
	if document.FixtureVersion != SupportedVersion {
		return nil, fixtureError(CodeVersion, "unsupported fixture version")
	}

	schema, err := loadReferencedFile(canonicalRoot, document.SchemaPath, "schema", maxSchemaBytes)
	if err != nil {
		return nil, err
	}
	configBytes, err := loadReferencedFile(canonicalRoot, document.CompilerConfigPath, "compiler config", maxConfigBytes)
	if err != nil {
		return nil, err
	}
	policyBytes, err := loadReferencedFile(canonicalRoot, document.PolicyIRPath, "PolicyIR", maxPolicyBytes)
	if err != nil {
		return nil, err
	}

	declaredDigest, err := parseSchemaDigest(document.SchemaSHA256)
	if err != nil {
		return nil, err
	}
	actualDigest := sha256.Sum256(schema)
	if !bytes.Equal(declaredDigest, actualDigest[:]) {
		return nil, fixtureError(CodeSchemaDigest, "declared schema digest does not match the exact schema bytes")
	}

	compilerConfig, err := config.Parse(configBytes)
	if err != nil {
		return nil, fixtureError(CodeProvenance, "compiler provenance config is invalid")
	}
	var program razielv1.PolicyProgram
	if err := (proto.UnmarshalOptions{DiscardUnknown: false}).Unmarshal(policyBytes, &program); err != nil {
		return nil, fixtureError(CodePolicyIR, "PolicyIR cannot be decoded")
	}
	if len(program.ProtoReflect().GetUnknown()) != 0 {
		return nil, fixtureError(CodePolicyIR, "PolicyIR contains unknown fields")
	}
	if !bytes.Equal(program.GetSourceSchemaDigest(), actualDigest[:]) {
		return nil, fixtureError(CodePolicyIR, "PolicyIR source digest does not match the exact schema bytes")
	}

	metadata, err := buildMetadata(&program, compilerConfig)
	if err != nil {
		return nil, err
	}
	fixture := &Fixture{
		Version:       document.FixtureVersion,
		Schema:        append([]byte(nil), schema...),
		Relationships: append([]Relationship(nil), document.Relationships...),
		Cases:         append([]Case(nil), document.Cases...),
	}
	if err := validateFixture(fixture, metadata, compilerConfig.Limits()); err != nil {
		return nil, err
	}
	sort.Slice(fixture.Relationships, func(i, j int) bool {
		return relationshipKey(fixture.Relationships[i]) < relationshipKey(fixture.Relationships[j])
	})
	return fixture, nil
}

func hasRepositoryMarkers(directory string) bool {
	for _, name := range []string{"AGENTS.md", "go.work"} {
		info, err := os.Stat(filepath.Join(directory, name))
		if err != nil || !info.Mode().IsRegular() {
			return false
		}
	}
	return true
}

func loadReferencedFile(repoRoot, relativePath, label string, maxBytes int64) ([]byte, error) {
	resolved, err := resolveConfinedFile(repoRoot, relativePath, false)
	if err != nil {
		return nil, err
	}
	return readFile(resolved, label, maxBytes)
}

func resolveConfinedFile(repoRoot, candidate string, allowAbsolute bool) (string, error) {
	if candidate == "" {
		return "", fixtureError(CodePath, "a required input path is empty")
	}
	if filepath.IsAbs(candidate) && !allowAbsolute {
		return "", fixtureError(CodePath, "referenced paths must be repository-relative")
	}
	target := candidate
	if !filepath.IsAbs(target) {
		target = filepath.Join(repoRoot, target)
		target = filepath.Clean(target)
		if !pathWithin(repoRoot, target) {
			return "", fixtureError(CodePath, "input path escapes the repository boundary")
		}
	}
	target = filepath.Clean(target)
	canonical, err := filepath.EvalSymlinks(target)
	if err != nil {
		return "", fixtureError(CodePath, "an input file cannot be resolved")
	}
	if !pathWithin(repoRoot, canonical) {
		return "", fixtureError(CodePath, "input path resolves outside the repository boundary")
	}
	info, err := os.Stat(canonical)
	if err != nil || !info.Mode().IsRegular() {
		return "", fixtureError(CodePath, "an input path is not a regular file")
	}
	return canonical, nil
}

func pathWithin(root, target string) bool {
	relative, err := filepath.Rel(root, target)
	if err != nil {
		return false
	}
	return relative != ".." && !strings.HasPrefix(relative, ".."+string(filepath.Separator))
}

func readFile(path, label string, maxBytes int64) ([]byte, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, fixtureError(CodePath, label+" file cannot be read")
	}
	defer file.Close()

	data, err := io.ReadAll(io.LimitReader(file, maxBytes+1))
	if err != nil {
		return nil, fixtureError(CodePath, label+" file cannot be read")
	}
	if int64(len(data)) > maxBytes {
		return nil, fixtureError(CodeLimit, label+" file exceeds its byte limit")
	}
	return data, nil
}

func parseSchemaDigest(encoded string) ([]byte, error) {
	if encoded == "" || encoded != strings.ToLower(encoded) {
		return nil, fixtureError(CodeSchemaDigest, "schema digest must be lowercase hexadecimal")
	}
	decoded, err := hex.DecodeString(encoded)
	if err != nil || len(decoded) != sha256.Size {
		return nil, fixtureError(CodeSchemaDigest, "schema digest must contain 32 bytes")
	}
	return decoded, nil
}
