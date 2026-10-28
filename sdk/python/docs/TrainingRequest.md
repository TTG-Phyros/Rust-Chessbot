# TrainingRequest

A training job, as a client asks for it.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**architecture** | [**Architecture**](Architecture.md) |  | [optional] 
**architecture_id** | **UUID** | A saved architecture, instead of &#x60;architecture&#x60;. | [optional] 
**datasets** | **List[UUID]** | Datasets to train on (ready ones). | 
**device** | [**Device**](Device.md) | Where it may run. | [optional] 
**hyperparameters** | [**Hyperparameters**](Hyperparameters.md) | Hyperparameters (defaults for the missing ones). | [optional] 
**name** | **str** | Name (default: the architecture&#39;s). | [optional] 
**seed** | **int** | Seed (default: drawn at random, then recorded). | [optional] 
**trainer** | **str** | Trainer backend. | [optional] 

## Example

```python
from chess_nnue_lab.models.training_request import TrainingRequest

# TODO update the JSON string below
json = "{}"
# create an instance of TrainingRequest from a JSON string
training_request_instance = TrainingRequest.from_json(json)
# print the JSON string representation of the object
print(TrainingRequest.to_json())

# convert the object into a dict
training_request_dict = training_request_instance.to_dict()
# create an instance of TrainingRequest from a dict
training_request_from_dict = TrainingRequest.from_dict(training_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


