# Architecture

A complete NNUE architecture: input features, feature transformer (accumulator), dense layers, output buckets and quantization.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**var_schema** | **str** | Optional link to this schema, for editors. Ignored. | [optional] 
**accumulator** | [**Accumulator**](Accumulator.md) | Feature transformer: the first, sparse layer, updated incrementally. | 
**features** | [**Features**](Features.md) | Input features of the network. | 
**hidden_layers** | [**List[DenseLayer]**](DenseLayer.md) | Dense layers between the accumulator and the output, in order. The output layer (one neuron, no activation) is implicit. | [optional] [default to []]
**metadata** | [**Metadata**](Metadata.md) | Descriptive information. Not part of the architecture hash. | 
**output_buckets** | [**OutputBuckets**](OutputBuckets.md) | Several sets of weights for the layers after the accumulator, one of which is chosen by the position. | [optional] 
**quantization** | [**Quantization**](Quantization.md) | How weights and activations are stored as integers. | 
**schema_version** | **int** | Version of the architecture schema. Always 1. | 

## Example

```python
from chess_nnue_lab.models.architecture import Architecture

# TODO update the JSON string below
json = "{}"
# create an instance of Architecture from a JSON string
architecture_instance = Architecture.from_json(json)
# print the JSON string representation of the object
print(Architecture.to_json())

# convert the object into a dict
architecture_dict = architecture_instance.to_dict()
# create an instance of Architecture from a dict
architecture_from_dict = Architecture.from_dict(architecture_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


