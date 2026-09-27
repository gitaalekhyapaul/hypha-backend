package composition

import (
	"github.com/authzed/spicedb/pkg/schemadsl/dslshape"
	"github.com/authzed/spicedb/pkg/schemadsl/input"
	"github.com/authzed/spicedb/pkg/schemadsl/parser"
)

type sourceNode struct {
	kind     dslshape.NodeType
	strings  map[string]string
	ints     map[string]int
	children []*sourceNode
}

func (n *sourceNode) Connect(predicate string, other parser.AstNode) {
	if predicate == dslshape.NodePredicateChild {
		n.children = append(n.children, other.(*sourceNode))
	}
}

func (n *sourceNode) MustDecorate(key, value string) parser.AstNode {
	n.strings[key] = value
	return n
}

func (n *sourceNode) MustDecorateWithInt(key string, value int) parser.AstNode {
	n.ints[key] = value
	return n
}

// Definition spans come from the pinned parser, not brace scanning or regex.
// v1.54.0 names these decorations "rune" positions but its lexer emits byte offsets.
// Preserve each definition verbatim, excluding only its leading documentation.
func definitionSources(source []byte) (map[string][]byte, error) {
	root := parser.Parse(func(_ input.Source, kind dslshape.NodeType) parser.AstNode {
		return &sourceNode{kind: kind, strings: map[string]string{}, ints: map[string]int{}}
	}, input.Source("package"), string(source)).(*sourceNode)
	result := map[string][]byte{}
	for _, node := range root.children {
		var name string
		switch node.kind {
		case dslshape.NodeTypeDefinition:
			name = node.strings[dslshape.NodeDefinitionPredicateName]
		case dslshape.NodeTypeCaveatDefinition:
			name = node.strings[dslshape.NodeCaveatDefinitionPredicateName]
		case dslshape.NodeTypeComment:
			continue
		default:
			return nil, ErrPolicy
		}
		start, startOK := node.ints[dslshape.NodePredicateStartRune]
		end, endOK := node.ints[dslshape.NodePredicateEndRune]
		if name == "" || !startOK || !endOK || start < 0 || end < start || end >= len(source) || result[name] != nil {
			return nil, ErrPolicy
		}
		result[name] = source[start : end+1]
	}
	return result, nil
}
