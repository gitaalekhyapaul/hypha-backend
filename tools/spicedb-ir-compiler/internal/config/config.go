package config

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"

	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
)

const supportedVersion uint32 = 1

// Config contains public compiler metadata. It never contains runtime caveat values.
type Config struct {
	limits  *razielv1.PolicyLimits
	caveats map[string]map[string]razielv1.CaveatInputSource
}

type fileConfig struct {
	ConfigVersion uint32                       `json:"config_version"`
	Limits        fileLimits                   `json:"limits"`
	Caveats       map[string]map[string]string `json:"caveats"`
}

type fileLimits struct {
	MaxExpressionNodes        uint32 `json:"max_expression_nodes"`
	MaxCaveatNodes            uint32 `json:"max_caveat_nodes"`
	MaxPathDepth              uint32 `json:"max_path_depth"`
	MaxRelationshipWitnesses  uint32 `json:"max_relationship_witnesses"`
	MaxMerkleDepth            uint32 `json:"max_merkle_depth"`
	MaxStringBytes            uint32 `json:"max_string_bytes"`
	MaxContextEntries         uint32 `json:"max_context_entries"`
	MaxTotalPrivateInputBytes uint32 `json:"max_total_private_input_bytes"`
}

// Parse decodes one strict JSON configuration value.
func Parse(data []byte) (Config, error) {
	if err := rejectDuplicateKeys(data); err != nil {
		return Config{}, fmt.Errorf("invalid compiler config: %w", err)
	}

	var file fileConfig
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&file); err != nil {
		return Config{}, fmt.Errorf("invalid compiler config: %w", err)
	}
	if err := requireEOF(decoder); err != nil {
		return Config{}, fmt.Errorf("invalid compiler config: %w", err)
	}
	if file.ConfigVersion != supportedVersion {
		return Config{}, fmt.Errorf("unsupported compiler config version %d", file.ConfigVersion)
	}
	if file.Caveats == nil {
		return Config{}, errors.New("compiler config must contain a caveats object")
	}

	limits := file.Limits.toProto()
	if err := validateLimits(limits); err != nil {
		return Config{}, err
	}

	caveats := make(map[string]map[string]razielv1.CaveatInputSource, len(file.Caveats))
	for caveatName, parameters := range file.Caveats {
		if caveatName == "" {
			return Config{}, errors.New("caveat binding name must not be empty")
		}
		converted := make(map[string]razielv1.CaveatInputSource, len(parameters))
		for parameterName, label := range parameters {
			if parameterName == "" {
				return Config{}, fmt.Errorf("caveat %q contains an empty parameter name", caveatName)
			}
			source, err := parseSource(label)
			if err != nil {
				return Config{}, fmt.Errorf("caveat %q parameter %q: %w", caveatName, parameterName, err)
			}
			converted[parameterName] = source
		}
		caveats[caveatName] = converted
	}

	return Config{limits: limits, caveats: caveats}, nil
}

// Limits returns an independent copy for one PolicyProgram.
func (c Config) Limits() *razielv1.PolicyLimits {
	if c.limits == nil {
		return nil
	}
	return &razielv1.PolicyLimits{
		MaxExpressionNodes:        c.limits.GetMaxExpressionNodes(),
		MaxCaveatNodes:            c.limits.GetMaxCaveatNodes(),
		MaxPathDepth:              c.limits.GetMaxPathDepth(),
		MaxRelationshipWitnesses:  c.limits.GetMaxRelationshipWitnesses(),
		MaxMerkleDepth:            c.limits.GetMaxMerkleDepth(),
		MaxStringBytes:            c.limits.GetMaxStringBytes(),
		MaxContextEntries:         c.limits.GetMaxContextEntries(),
		MaxTotalPrivateInputBytes: c.limits.GetMaxTotalPrivateInputBytes(),
	}
}

// CaveatBindings returns an independent copy of the configured provenance labels.
func (c Config) CaveatBindings() map[string]map[string]razielv1.CaveatInputSource {
	copy := make(map[string]map[string]razielv1.CaveatInputSource, len(c.caveats))
	for caveatName, parameters := range c.caveats {
		parameterCopy := make(map[string]razielv1.CaveatInputSource, len(parameters))
		for parameterName, source := range parameters {
			parameterCopy[parameterName] = source
		}
		copy[caveatName] = parameterCopy
	}
	return copy
}

func (l fileLimits) toProto() *razielv1.PolicyLimits {
	return &razielv1.PolicyLimits{
		MaxExpressionNodes:        l.MaxExpressionNodes,
		MaxCaveatNodes:            l.MaxCaveatNodes,
		MaxPathDepth:              l.MaxPathDepth,
		MaxRelationshipWitnesses:  l.MaxRelationshipWitnesses,
		MaxMerkleDepth:            l.MaxMerkleDepth,
		MaxStringBytes:            l.MaxStringBytes,
		MaxContextEntries:         l.MaxContextEntries,
		MaxTotalPrivateInputBytes: l.MaxTotalPrivateInputBytes,
	}
}

func validateLimits(limits *razielv1.PolicyLimits) error {
	for _, limit := range []struct {
		name  string
		value uint32
	}{
		{name: "max_expression_nodes", value: limits.GetMaxExpressionNodes()},
		{name: "max_caveat_nodes", value: limits.GetMaxCaveatNodes()},
		{name: "max_path_depth", value: limits.GetMaxPathDepth()},
		{name: "max_relationship_witnesses", value: limits.GetMaxRelationshipWitnesses()},
		{name: "max_merkle_depth", value: limits.GetMaxMerkleDepth()},
		{name: "max_string_bytes", value: limits.GetMaxStringBytes()},
		{name: "max_context_entries", value: limits.GetMaxContextEntries()},
		{name: "max_total_private_input_bytes", value: limits.GetMaxTotalPrivateInputBytes()},
	} {
		if limit.value == 0 {
			return fmt.Errorf("compiler config limit %q must be greater than zero", limit.name)
		}
	}
	return nil
}

func parseSource(label string) (razielv1.CaveatInputSource, error) {
	switch label {
	case "REQUEST_CONTEXT":
		return razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_REQUEST_CONTEXT, nil
	case "RELATIONSHIP_CONTEXT":
		return razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_RELATIONSHIP_CONTEXT, nil
	case "CAPABILITY_CLAIM":
		return razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_CAPABILITY_CLAIM, nil
	default:
		return razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_UNSPECIFIED,
			fmt.Errorf("unknown input source %q", label)
	}
}

func requireEOF(decoder *json.Decoder) error {
	var extra any
	if err := decoder.Decode(&extra); errors.Is(err, io.EOF) {
		return nil
	} else if err != nil {
		return err
	}
	return errors.New("compiler config must contain one JSON value")
}

func rejectDuplicateKeys(data []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	if err := walkJSONValue(decoder); err != nil {
		return err
	}
	return requireEOF(decoder)
}

func walkJSONValue(decoder *json.Decoder) error {
	token, err := decoder.Token()
	if err != nil {
		return err
	}
	delimiter, ok := token.(json.Delim)
	if !ok {
		return nil
	}

	switch delimiter {
	case '{':
		keys := map[string]struct{}{}
		for decoder.More() {
			keyToken, err := decoder.Token()
			if err != nil {
				return err
			}
			key, ok := keyToken.(string)
			if !ok {
				return errors.New("JSON object key is not a string")
			}
			if _, exists := keys[key]; exists {
				return fmt.Errorf("duplicate JSON key %q", key)
			}
			keys[key] = struct{}{}
			if err := walkJSONValue(decoder); err != nil {
				return err
			}
		}
		_, err = decoder.Token()
		return err
	case '[':
		for decoder.More() {
			if err := walkJSONValue(decoder); err != nil {
				return err
			}
		}
		_, err = decoder.Token()
		return err
	default:
		return fmt.Errorf("unexpected JSON delimiter %q", delimiter)
	}
}
