<script setup lang="ts">
    import { reactive, ref } from 'vue';
    import { ElMessage } from 'element-plus';
    import AccountAuthForm from '../../components/AccountAuthForm.vue';
    import { open } from '@tauri-apps/plugin-dialog';
    import { invoke } from '@tauri-apps/api/core';
    import { formatObjectString, handleLocalAccount } from '../../utils/function'
    import { openUrl } from '@tauri-apps/plugin-opener';

    const faceName = ref('');
    const threshold = ref(50);
    // 显示的图片
    const capturedImage = ref('');
    // 这是用来保存的，不要显示
    let rawImageForSystem = '';
    // 是否是摄像头模式
    const isCameraStreaming = ref(false);
    // 是否启用raf循环
    let isLoopRunning = false;
    // 一致性验证模式开关
    const verificationMode = ref(false);
    // 一致性验证模式下的图片
    const verifyingStreamImage = ref('');
    const matchConfidence = ref(0);
    const isProcessing = ref(false);
    // const faceDetectionThreshold = ref(90); // cy:  人脸识别度，暂时废弃，等设置页面写好在说

    let authForm = reactive({
        accountType: 'local',
        username: '',
        password: ''
    });

    // 获取当前用户名
    invoke('get_now_username').then((data)=>{
        if(data.code == 200){
            authForm.username = data.data.username;
        }
    })

    const handleSelectFile = async () => {
        try {
            const selected = await open({
                multiple: false,
                directory: false,
                filters: [{ name: '图片文件', extensions: ['jpg', 'jpeg', 'png'] }]
            });

            if (!selected) return; 

            isProcessing.value = true;
            
            const result = await invoke("check_face_from_img", { imgPath: selected });
            
            capturedImage.value = result.data.display_base64;
            rawImageForSystem = result.data.raw_base64;

            ElMessage.success('图片载入成功');
        } catch (error) {
            ElMessage.error(formatObjectString(error));
            console.error(error);
        } finally {
            isProcessing.value = false;
        }
    };

    const startCamera = () => {
        invoke("open_camera").then(()=>{
            isCameraStreaming.value = true;
            isLoopRunning = true;
            streamLoop();
        }).catch((error)=>{
            ElMessage.error(formatObjectString(error));
            console.error(error);
        });
    };

    const streamLoop = async () => {
        if (!isLoopRunning) return;

        try {
            if(!verificationMode.value){
                // 面容录入
                const res = await invoke('check_face_from_camera');
                capturedImage.value = res.data.display_base64;
                rawImageForSystem = res.data.raw_base64;
            } else {
                // 一致性对比
                const res = await invoke('verify_face', { referenceBase64: rawImageForSystem.split(',')[1] });
                if(res.data.display_base64) {
                    verifyingStreamImage.value = res.data.display_base64;
                }

                const rawScore = res.data.score;
                if (rawScore > 0) {
                    matchConfidence.value = Math.floor(Math.min(100, (rawScore / 1.0) * 100));
                } else {
                    matchConfidence.value = 0;
                }
            }

            // 继续下一帧
            requestAnimationFrame(streamLoop);
        } catch (error) {
            const info = formatObjectString(error);
            if(info.includes("未检测到人脸")){
                // 这个可以继续，并且不用显示错误
                requestAnimationFrame(streamLoop);
                return;
            }
            ElMessage.error(info);
        }
    };

    const confirmCapture = () => {
        stopCamera().then(()=>{
            isCameraStreaming.value = false;
        }).catch(()=>{});
    };

    const stopCapture = () => {
        stopCamera().then(()=>{
            isCameraStreaming.value = false;
            capturedImage.value = '';
            rawImageForSystem = '';
        }).catch(()=>{});
    };

    function stopCamera(){
        isLoopRunning = false;
        return new Promise((resolve, reject) => {
            invoke("stop_camera").then(()=>{
                resolve();
            }).catch((error)=>{
                ElMessage.error(formatObjectString(error));
                console.error(error);
                reject();
            });
        })
    }

    // 切换验证模式
    const toggleVerification = () => {
        verificationMode.value = !verificationMode.value;
        if (verificationMode.value) {
            invoke("open_camera").then(()=>{
                isLoopRunning = true;
                streamLoop();
            }).catch((error)=>{
                ElMessage.error(formatObjectString(error));
                console.error(error);
            });
        } else {
            stopCamera().then(()=>{
                verifyingStreamImage.value = '';
            }).catch(()=>{});
        }
    };

    const handleSave = () => {
        if (!authForm.username || !authForm.password) {
            ElMessage.warning('请填写完整的账号密码信息')
            return;
        }

        if (!rawImageForSystem) {
            ElMessage.warning('请先录入面容图片');
            return;
        }

        isProcessing.value = true;

        invoke("save_face_registration", {name: faceName.value || '', referenceBase64: rawImageForSystem.split(',')[1]}).then((result)=>{
            console.log('特征已保存至文件:', result);
        }).catch((error)=>{
            ElMessage.error(formatObjectString(error));
            console.error(error);
        }).finally(()=>{
            isProcessing.value = false;
        });
    };
</script>

<template>
    <div class="face-add-container">
        <el-row :gutter="24">
            <el-col :span="14">
                <el-card class="visual-card" shadow="never">
                    <div class="display-container" :class="{ 'split-view': verificationMode }">

                        <div class="screen-box primary-screen">
                            <div class="screen-label">{{ verificationMode ? '参考底库' : '采集预览' }}</div>
                            <div v-if="!capturedImage" class="placeholder-content">
                                <el-icon :size="48">
                                    <UserFilled />
                                </el-icon>
                                <p>待录入面容</p>
                            </div>
                            <img v-else :src="capturedImage" class="result-img" />
                        </div>

                        <div v-if="verificationMode" class="screen-box secondary-screen">
                            <div class="screen-label">实时验证流</div>
                            <div class="scanner-line"></div>
                            <div v-if="!verifyingStreamImage" class="camera-stream-mock">
                                <el-icon :size="48" class="is-loading">
                                    <Loading />
                                </el-icon>
                            </div>
                            <img v-else :src="verifyingStreamImage" class="result-img" />
                            <div class="confidence-tag" :class="matchConfidence > (threshold) ? 'match' : 'mismatch'">
                                相似度: {{ matchConfidence }}%
                            </div>
                        </div>
                    </div>

                    <div class="action-bar">
                        <!-- cy: 人脸识别度，暂时废弃，等设置页面写好在说 -->
                        <!-- <div class="detection-config">
                            <span class="label">检测灵敏度</span>
                            <el-slider 
                                v-model="faceDetectionThreshold" 
                                :min="10" 
                                :max="100" 
                                size="small"
                            />
                            <el-tooltip content="控制摄像头识别出人脸的难易程度" placement="top">
                                <el-icon :size="14" style="margin-left: 5px; cursor: help;"><QuestionFilled /></el-icon>
                            </el-tooltip>
                        </div> -->
                        <div class="capture-controls" v-if="!verificationMode">
                            <template v-if="!isCameraStreaming">
                                <el-button 
                                    type="primary" 
                                    plain 
                                    icon="Picture" 
                                    @click="handleSelectFile"
                                    :loading="isProcessing"
                                >
                                    选择本地照片
                                </el-button>
                                <el-button type="primary" @click="startCamera" :loading="isProcessing">从摄像头抓拍</el-button>
                            </template>
                            <template v-else>
                                <el-button type="success" icon="Check" @click="confirmCapture">确认抓拍</el-button>
                                <el-button type="danger" plain icon="Close" @click="stopCapture">取消</el-button>
                            </template>
                        </div>

                        <div class="verify-controls" v-else>
                            <el-tag type="info" effect="plain">正在进行一致性验证...</el-tag>
                        </div>

                        <el-button v-if="capturedImage && !isCameraStreaming" :type="verificationMode ? 'danger' : 'warning'"
                            @click="toggleVerification">
                            {{ verificationMode ? '停止验证' : '一致性验证' }}
                        </el-button>
                    </div>
                </el-card>
            </el-col>

            <el-col :span="10">
                <el-card shadow="never">
                    <template #header><span class="font-bold">底库配置</span></template>
                    <el-form label-position="top">
                        <el-form-item label="面容别名">
                            <el-input v-model="faceName" placeholder="如：XX设备录入" />
                        </el-form-item>

                        <el-form-item label="判定阈值 (置信度)">
                            <el-slider v-model="threshold" :min="20" :max="100" />
                            <div class="tip">
                                当前阈值: <b style="color: #606266; margin: 0 4px;">{{ threshold }}%</b>
                                <span @click="openUrl('https://docs.opencv.org/4.x/d0/dd4/tutorial_dnn_face.html')">
                                    OpenCV 官网建议 ≥ 0.363 (约 36%)
                                </span>
                            </div>
                        </el-form-item>

                        <el-divider>关联系统账户</el-divider>
                        <AccountAuthForm v-model="authForm" />

                        <div class="footer-btns">
                            <el-button type="success" size="large" @click="handleSave" :disabled="!capturedImage || isCameraStreaming" :loading="isProcessing">
                                保存并录入系统
                            </el-button>
                        </div>
                    </el-form>
                </el-card>
            </el-col>
        </el-row>
    </div>
</template>

<style scoped>
    .display-container {
        display: flex;
        gap: 10px;
        height: 320px;
        background: #000;
        border-radius: 8px;
        overflow: hidden;
        transition: all 0.3s ease;
    }

    .screen-box {
        flex: 1;
        position: relative;
        display: flex;
        justify-content: center;
        align-items: center;
        background: #1a1a1a;
        border: 1px solid #333;
    }

    .screen-label {
        position: absolute;
        top: 10px;
        left: 10px;
        background: rgba(0, 0, 0, 0.6);
        color: #fff;
        padding: 2px 8px;
        font-size: 12px;
        border-radius: 4px;
        z-index: 5;
    }

    .result-img {
        max-width: 100%;
        max-height: 100%;
        object-fit: contain;
        filter: drop-shadow(0 0 8px rgba(0, 242, 255, 0.2));
        border: 1px solid #333;
    }

    .placeholder-content {
        color: #444;
        text-align: center;
    }

    /* 验证模式下的分割线效果 */
    .split-view .screen-box {
        flex: 0 0 calc(50% - 5px);
    }

    .camera-stream-mock {
        width: 100%;
        height: 100%;
        display: flex;
        justify-content: center;
        align-items: center;
        color: #409eff;
    }

    .scanner-line {
        position: absolute;
        width: 100%;
        height: 2px;
        background: rgba(64, 158, 255, 0.5);
        box-shadow: 0 0 10px #409eff;
        animation: scan 2s infinite ease-in-out;
    }

    .confidence-tag {
        position: absolute;
        bottom: 20px;
        padding: 5px 15px;
        border-radius: 20px;
        font-weight: bold;
        font-size: 14px;
    }

    .match {
        background: #67c23a;
        color: white;
    }

    .mismatch {
        background: #f56c6c;
        color: white;
    }

    .detection-config {
        display: flex;
        align-items: center;
        background: #f0f2f5;
        padding: 5px 12px;
        border-radius: 4px;
        gap: 10px;
        width: 100%;
    }

    .detection-config .label {
        font-size: 12px;
        color: #606266;
        white-space: nowrap;
    }

    .action-bar {
        margin-top: 20px;
        display: flex;
        justify-content: space-between;
        align-items: center;
        flex-wrap: wrap;
        gap: 10px;
    }

    .footer-btns {
        margin-top: 20px;
    }

    .tip {
        font-size: 13px;
        color: #909399;
        margin-top: 8px;
        display: flex;
        align-items: center;
    }

    .tip span {
        margin-left: 8px;
        color: #409eff;
        cursor: pointer;
        text-decoration: underline;
        transition: color 0.2s ease;
        text-underline-offset: 3px;
    }

    .tip span:hover {
        color: #66b1ff;
        text-decoration: none;
    }

    @keyframes scan {
        0% {
            top: 10%;
        }

        50% {
            top: 90%;
        }

        100% {
            top: 10%;
        }
    }
</style>