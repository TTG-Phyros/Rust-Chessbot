# TrainingJob

A training job, fully resolved: everything needed to reproduce it.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**architecture** | [**Architecture**](Architecture.md) |  | 
**architecture_hash** | **str** | Its hash. | 
**datasets** | [**List[DatasetRef]**](DatasetRef.md) | Training data. | 
**device** | [**Device**](Device.md) | Where it may run. | 
**hyperparameters** | [**Hyperparameters**](Hyperparameters.md) | Hyperparameters. | 
**seed** | **int** | Seed of the initialization and of the data order. | 
**trainer** | **str** | Trainer backend (&#x60;burn&#x60;). | 

## Example

```python
from chess_nnue_lab.models.training_job import TrainingJob

# TODO update the JSON string below
json = "{}"
# create an instance of TrainingJob from a JSON string
training_job_instance = TrainingJob.from_json(json)
# print the JSON string representation of the object
print(TrainingJob.to_json())

# convert the object into a dict
training_job_dict = training_job_instance.to_dict()
# create an instance of TrainingJob from a dict
training_job_from_dict = TrainingJob.from_dict(training_job_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


