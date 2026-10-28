# Preset

A ready-made architecture.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**architecture** | [**Architecture**](Architecture.md) |  | 
**estimate** | [**Estimate**](Estimate.md) |  | 
**hash** | **str** | Hash of the architecture. | 
**id** | **str** | Identifier, e.g. &#x60;fast-768&#x60;. | 

## Example

```python
from chess_nnue_lab.models.preset import Preset

# TODO update the JSON string below
json = "{}"
# create an instance of Preset from a JSON string
preset_instance = Preset.from_json(json)
# print the JSON string representation of the object
print(Preset.to_json())

# convert the object into a dict
preset_dict = preset_instance.to_dict()
# create an instance of Preset from a dict
preset_from_dict = Preset.from_dict(preset_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


