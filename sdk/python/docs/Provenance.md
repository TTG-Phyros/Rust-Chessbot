# Provenance

How a model was made: enough to train it again.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**datasets** | [**List[DatasetRef]**](DatasetRef.md) | Training data, with content hashes. | 
**device** | **str** | Device it was trained on. | 
**final_loss** | **float** | Loss of the last superbatch. | [optional] 
**hyperparameters** | [**Hyperparameters**](Hyperparameters.md) | Hyperparameters. | 
**positions** | **int** | Positions trained on. | 
**seed** | **int** | Seed. | 
**superbatches** | **int** | Superbatches trained. | 
**trainer** | **str** | Trainer backend. | 

## Example

```python
from chess_nnue_lab.models.provenance import Provenance

# TODO update the JSON string below
json = "{}"
# create an instance of Provenance from a JSON string
provenance_instance = Provenance.from_json(json)
# print the JSON string representation of the object
print(Provenance.to_json())

# convert the object into a dict
provenance_dict = provenance_instance.to_dict()
# create an instance of Provenance from a dict
provenance_from_dict = Provenance.from_dict(provenance_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


