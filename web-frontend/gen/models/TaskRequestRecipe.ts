/* tslint:disable */
/* eslint-disable */
/**
 * SegmentedEncoder rest api
 * No description provided (generated by Openapi Generator https://github.com/openapitools/openapi-generator)
 *
 * The version of the OpenAPI document: 0.0.3
 * 
 *
 * NOTE: This class is auto generated by OpenAPI Generator (https://openapi-generator.tech).
 * https://openapi-generator.tech
 * Do not edit the class manually.
 */

import type { MergeTask } from './MergeTask';
import {
    instanceOfMergeTask,
    MergeTaskFromJSON,
    MergeTaskFromJSONTyped,
    MergeTaskToJSON,
} from './MergeTask';
import type { TranscodeTask } from './TranscodeTask';
import {
    instanceOfTranscodeTask,
    TranscodeTaskFromJSON,
    TranscodeTaskFromJSONTyped,
    TranscodeTaskToJSON,
} from './TranscodeTask';

/**
 * @type TaskRequestRecipe
 * 
 * @export
 */
export type TaskRequestRecipe = MergeTask | TranscodeTask;

export function TaskRequestRecipeFromJSON(json: any): TaskRequestRecipe {
    return TaskRequestRecipeFromJSONTyped(json, false);
}

export function TaskRequestRecipeFromJSONTyped(json: any, ignoreDiscriminator: boolean): TaskRequestRecipe {
    if (json == null) {
        return json;
    }
    return { ...MergeTaskFromJSONTyped(json, true), ...TranscodeTaskFromJSONTyped(json, true) };
}

export function TaskRequestRecipeToJSON(value?: TaskRequestRecipe | null): any {
    if (value == null) {
        return value;
    }

    if (instanceOfMergeTask(value)) {
        return MergeTaskToJSON(value as MergeTask);
    }
    if (instanceOfTranscodeTask(value)) {
        return TranscodeTaskToJSON(value as TranscodeTask);
    }

    return {};
}
