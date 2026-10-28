# Accumulator

The feature transformer.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**activation** | [**Activation**](Activation.md) | Applied to each perspective before the dense layers. | 
**size** | **int** | Neurons per perspective; a multiple of 16 (32 with &#x60;PairwiseCReLU&#x60;). | 

## Example

```python
from chess_nnue_lab.models.accumulator import Accumulator

# TODO update the JSON string below
json = "{}"
# create an instance of Accumulator from a JSON string
accumulator_instance = Accumulator.from_json(json)
# print the JSON string representation of the object
print(Accumulator.to_json())

# convert the object into a dict
accumulator_dict = accumulator_instance.to_dict()
# create an instance of Accumulator from a dict
accumulator_from_dict = Accumulator.from_dict(accumulator_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


